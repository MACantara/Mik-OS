//! Mik-64 kernel builder for the MVP.

use std::env;
use std::fs;

fn main() {
    let out = env::args()
        .nth(1)
        .unwrap_or_else(|| "mik-64-kernel.bin".to_string());
    let binary = mik_os::kernel_binary();
    fs::write(&out, &binary).expect("failed to write kernel binary");
    println!("wrote {} ({} bytes)", out, binary.len());
}
