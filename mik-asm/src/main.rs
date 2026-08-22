use std::env;
use std::fs;

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        return Err("usage: mik-asm <input.s> <output.bin>".to_string());
    }

    let source = fs::read_to_string(&args[1])
        .map_err(|e| format!("failed to read {}: {}", args[1], e))?;
    let binary = mik_asm::assemble(&source, 0x400000)?;
    fs::write(&args[2], binary).map_err(|e| format!("failed to write {}: {}", args[2], e))?;
    Ok(())
}
