use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("build") => build_kernel(),
        Some("qemu") | Some("run") => run_kernel(),
        _ => {
            eprintln!("usage: mik-os-x86 <build|qemu>");
            std::process::exit(1);
        }
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("manifest has parent")
        .to_path_buf()
}

fn cargo() -> String {
    env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

fn kernel_elf() -> PathBuf {
    workspace_root()
        .join("target")
        .join("x86_64-unknown-none")
        .join("debug")
        .join("mik-os-x86-kernel")
}

fn build_kernel() {
    let ws = workspace_root();
    let mut cmd = Command::new(cargo());
    cmd.current_dir(&ws)
        .arg("build")
        .arg("-p")
        .arg("mik-os-x86-kernel")
        .arg("--target")
        .arg("x86_64-unknown-none");
    let status = cmd.status().expect("failed to run cargo build");
    if !status.success() {
        eprintln!("kernel build failed");
        std::process::exit(1);
    }
}

fn find_qemu() -> PathBuf {
    if let Ok(p) = env::var("QEMU") {
        return PathBuf::from(p);
    }
    if let Ok(output) = Command::new("where").arg("qemu-system-x86_64").output() {
        let s = String::from_utf8_lossy(&output.stdout);
        let line = s.lines().next().unwrap_or("").trim();
        if !line.is_empty() && Path::new(line).exists() {
            return PathBuf::from(line);
        }
    }
    let candidates = [
        r"C:\Program Files\qemu\qemu-system-x86_64.exe",
        r"C:\Program Files (x86)\qemu\qemu-system-x86_64.exe",
    ];
    for c in &candidates {
        if Path::new(c).exists() {
            return PathBuf::from(c);
        }
    }
    eprintln!("qemu-system-x86_64 not found; set QEMU or add it to PATH");
    std::process::exit(1);
}

fn run_kernel() {
    build_kernel();
    let elf = kernel_elf();
    if !elf.exists() {
        eprintln!("kernel ELF not found at {}", elf.display());
        std::process::exit(1);
    }
    let qemu = find_qemu();
    let mut cmd = Command::new(qemu);
    cmd.arg("-kernel").arg(elf)
        .arg("-serial").arg("stdio")
        .arg("-no-reboot")
        .arg("-no-shutdown");
    let status = cmd.status().expect("failed to run qemu");
    std::process::exit(status.code().unwrap_or(1));
}
