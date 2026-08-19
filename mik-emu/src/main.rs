use std::env;
use std::fs;
use std::io::{self, Write};

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: mik-emu <flat-binary>");
        return Err("missing binary".into());
    }
    let path = &args[1];
    let binary = fs::read(path).map_err(|e| format!("cannot read {}: {}", path, e))?;
    let mut out = io::stdout().lock();
    mik_emu::run(&binary, &mut out)?;
    out.flush().map_err(|e| e.to_string())?;
    Ok(())
}
