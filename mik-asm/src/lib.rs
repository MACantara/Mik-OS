//! Tiny text-to-binary assembler for the Mik-64 instruction set.
//!
//! This is the first slice of the Milestone 1.4 build chain. It supports a
//! minimal line-oriented syntax: opcodes, registers, numeric immediates,
//! labels, `.string`, and `#` line comments. Two-pass assembly resolves labels
//! and branch offsets.

use mik_emu::encode;
use std::collections::HashMap;

enum Item {
    Ins { op: String, args: Vec<String> },
    Data(Vec<u8>),
}

/// Assemble a Mik-64 source string into a flat binary starting at `base`.
pub fn assemble(source: &str, base: u64) -> Result<Vec<u8>, String> {
    let mut items: Vec<Item> = Vec::new();
    let mut labels: HashMap<String, usize> = HashMap::new();

    // First pass: tokenise, record labels, and collect instructions/data.
    for (line_no, line) in source.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        let mut tokens: Vec<&str> = line
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|s| !s.is_empty())
            .collect();
        if tokens.is_empty() {
            continue;
        }

        // A leading `label:` records the label for the following item.
        if tokens[0].ends_with(':') {
            let label = tokens[0].trim_end_matches(':').to_string();
            labels.insert(label, items.len());
            tokens.remove(0);
            if tokens.is_empty() {
                continue;
            }
        }

        if tokens[0].starts_with('.') {
            match tokens[0] {
                ".string" => {
                    if tokens.len() < 2 {
                        return Err(format!(
                            "line {}: .string expects a string literal",
                            line_no + 1
                        ));
                    }
                    let raw = tokens[1..].join(" ");
                    items.push(Item::Data(parse_string(&raw, line_no + 1)?));
                }
                _ => {
                    return Err(format!(
                        "line {}: unknown directive {}",
                        line_no + 1,
                        tokens[0]
                    ))
                }
            }
        } else {
            let op = tokens[0].to_lowercase();
            let args = tokens[1..].iter().map(|s| s.to_string()).collect();
            items.push(Item::Ins { op, args });
        }
    }

    // Compute the address of every item in the final binary.
    let mut item_addrs: Vec<u64> = Vec::with_capacity(items.len());
    let mut addr = base;
    for item in &items {
        item_addrs.push(addr);
        match item {
            Item::Ins { .. } => addr += 8,
            Item::Data(b) => addr += b.len() as u64,
        }
    }

    // Resolve labels to absolute addresses.
    let mut label_addrs: HashMap<String, u64> = HashMap::new();
    for (label, &idx) in &labels {
        label_addrs.insert(label.clone(), item_addrs[idx]);
    }

    // Second pass: encode instructions and collect data bytes.
    let mut binary: Vec<u8> = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        match item {
            Item::Ins { op, args } => {
                let word = encode_instruction(op, args, &label_addrs, item_addrs[idx])?;
                binary.extend_from_slice(&word.to_le_bytes());
            }
            Item::Data(b) => binary.extend_from_slice(b),
        }
    }

    Ok(binary)
}

fn encode_instruction(
    op: &str,
    args: &[String],
    labels: &HashMap<String, u64>,
    pc: u64,
) -> Result<u64, String> {
    let reg = |tok: &str| parse_register(tok);
    let imm = |tok: &str| parse_value(tok, labels, pc);

    match op {
        "halt" => Ok(encode(0x00, 0, 0, 0, 0)),

        "li" => {
            if args.len() != 2 {
                return Err(format!("li: expected 2 args, got {}", args.len()));
            }
            Ok(encode(0x01, reg(&args[0])?, 0, 0, imm(&args[1])?))
        }

        "add" => alu(0x02, args),
        "addi" => alu_imm(0x03, args),
        "sub" => alu(0x04, args),
        "and" => alu(0x05, args),
        "or" => alu(0x06, args),

        "load8" => load(0x07, args),
        "load64" => load(0x08, args),
        "store8" => store(0x09, args),
        "store64" => store(0x0A, args),

        "beq" => branch(0x0B, args, labels, pc),
        "bne" => branch(0x0C, args, labels, pc),

        "jmp" => {
            if args.len() != 1 {
                return Err(format!("jmp: expected 1 arg, got {}", args.len()));
            }
            let offset = parse_jump_target(&args[0], labels, pc)?;
            Ok(encode(0x0D, 0, 0, 0, offset))
        }

        "trap" => {
            if args.len() != 1 {
                return Err(format!("trap: expected 1 arg, got {}", args.len()));
            }
            Ok(encode(0x0E, 0, 0, 0, parse_number(&args[0])?))
        }

        "jmpr" => {
            if args.len() != 1 {
                return Err(format!("jmpr: expected 1 arg, got {}", args.len()));
            }
            Ok(encode(0x0F, 0, reg(&args[0])?, 0, 0))
        }

        "eret" => Ok(encode(0x10, 0, 0, 0, 0)),

        "rdcsr" => {
            if args.len() != 2 {
                return Err(format!("rdcsr: expected 2 args, got {}", args.len()));
            }
            Ok(encode(0x11, reg(&args[0])?, 0, 0, parse_number(&args[1])?))
        }

        "wrcsr" => {
            if args.len() != 2 {
                return Err(format!("wrcsr: expected 2 args, got {}", args.len()));
            }
            Ok(encode(0x12, 0, reg(&args[0])?, 0, parse_number(&args[1])?))
        }

        "sfence" => Ok(encode(0x13, 0, 0, 0, 0)),

        "sret" => {
            if args.len() != 1 {
                return Err(format!("sret: expected 1 arg, got {}", args.len()));
            }
            Ok(encode(0x14, 0, reg(&args[0])?, 0, 0))
        }

        "int" => {
            if args.len() != 1 {
                return Err(format!("int: expected 1 arg, got {}", args.len()));
            }
            Ok(encode(0x15, 0, 0, 0, parse_number(&args[0])?))
        }

        "iret" => Ok(encode(0x16, 0, 0, 0, 0)),

        _ => Err(format!("unknown opcode: {}", op)),
    }
}

fn alu(opcode: u8, args: &[String]) -> Result<u64, String> {
    if args.len() != 3 {
        return Err(format!("{}: expected 3 args, got {}", opcode, args.len()));
    }
    Ok(encode(
        opcode,
        parse_register(&args[0])?,
        parse_register(&args[1])?,
        parse_register(&args[2])?,
        0,
    ))
}

fn alu_imm(opcode: u8, args: &[String]) -> Result<u64, String> {
    if args.len() != 3 {
        return Err(format!("{}: expected 3 args, got {}", opcode, args.len()));
    }
    Ok(encode(
        opcode,
        parse_register(&args[0])?,
        parse_register(&args[1])?,
        0,
        parse_value(&args[2], &HashMap::new(), 0)?,
    ))
}

fn load(opcode: u8, args: &[String]) -> Result<u64, String> {
    if args.len() != 3 {
        return Err(format!("{}: expected 3 args, got {}", opcode, args.len()));
    }
    Ok(encode(
        opcode,
        parse_register(&args[0])?,
        parse_register(&args[1])?,
        0,
        parse_value(&args[2], &HashMap::new(), 0)?,
    ))
}

fn store(opcode: u8, args: &[String]) -> Result<u64, String> {
    if args.len() != 3 {
        return Err(format!("{}: expected 3 args, got {}", opcode, args.len()));
    }
    Ok(encode(
        opcode,
        0,
        parse_register(&args[0])?,
        parse_register(&args[1])?,
        parse_value(&args[2], &HashMap::new(), 0)?,
    ))
}

fn branch(
    opcode: u8,
    args: &[String],
    labels: &HashMap<String, u64>,
    pc: u64,
) -> Result<u64, String> {
    if args.len() != 3 {
        return Err(format!("{}: expected 3 args, got {}", opcode, args.len()));
    }
    Ok(encode(
        opcode,
        0,
        parse_register(&args[0])?,
        parse_register(&args[1])?,
        parse_jump_target(&args[2], labels, pc)?,
    ))
}

fn parse_register(tok: &str) -> Result<u8, String> {
    if tok.eq_ignore_ascii_case("sp") {
        return Ok(15);
    }
    if !tok.starts_with('x') && !tok.starts_with('X') {
        return Err(format!("expected register, got {}", tok));
    }
    tok[1..]
        .parse::<u8>()
        .map_err(|_| format!("invalid register: {}", tok))
        .and_then(|n| {
            if n > 15 {
                Err(format!("register out of range: {}", tok))
            } else {
                Ok(n)
            }
        })
}

fn parse_value(tok: &str, labels: &HashMap<String, u64>, pc: u64) -> Result<i64, String> {
    if let Some(&addr) = labels.get(tok) {
        Ok(addr as i64)
    } else {
        parse_number(tok).or_else(|_| parse_jump_target(tok, labels, pc))
    }
}

fn parse_jump_target(tok: &str, labels: &HashMap<String, u64>, pc: u64) -> Result<i64, String> {
    let target = if let Some(&addr) = labels.get(tok) {
        addr
    } else {
        parse_number(tok)? as u64
    };
    Ok((target as i64 - pc as i64) / 8)
}

fn parse_number(tok: &str) -> Result<i64, String> {
    if tok.starts_with("0x") || tok.starts_with("0X") {
        i64::from_str_radix(&tok[2..], 16).map_err(|e| format!("invalid hex {}: {}", tok, e))
    } else if tok.starts_with("0b") || tok.starts_with("0B") {
        i64::from_str_radix(&tok[2..], 2).map_err(|e| format!("invalid binary {}: {}", tok, e))
    } else if tok.starts_with('\'') && tok.ends_with('\'') && tok.len() == 3 {
        Ok(tok.as_bytes()[1] as i64)
    } else if tok.starts_with('\'') && tok.ends_with('\'') && tok.len() == 4 && tok.as_bytes()[1] == b'\\'
    {
        // A minimal escape: '\n'.
        match tok.as_bytes()[2] {
            b'n' => Ok(b'\n' as i64),
            b'r' => Ok(b'\r' as i64),
            b't' => Ok(b'\t' as i64),
            b'\\' => Ok(b'\\' as i64),
            b'0' => Ok(0),
            _ => Err(format!("unknown char escape: {}", tok)),
        }
    } else {
        tok.parse::<i64>()
            .map_err(|e| format!("invalid number {}: {}", tok, e))
    }
}

fn parse_string(raw: &str, line_no: usize) -> Result<Vec<u8>, String> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('"') || !trimmed.ends_with('"') {
        return Err(format!(
            "line {}: .string argument must be a quoted string",
            line_no
        ));
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    let mut out = Vec::new();
    let mut iter = inner.bytes();
    while let Some(b) = iter.next() {
        if b == b'\\' {
            match iter.next() {
                Some(b'n') => out.push(b'\n'),
                Some(b'r') => out.push(b'\r'),
                Some(b't') => out.push(b'\t'),
                Some(b'\\') => out.push(b'\\'),
                Some(b'0') => out.push(0),
                Some(c) => out.push(c),
                None => return Err(format!("line {}: trailing backslash", line_no)),
            }
        } else {
            out.push(b);
        }
    }
    out.push(0); // null terminate
    Ok(out)
}
