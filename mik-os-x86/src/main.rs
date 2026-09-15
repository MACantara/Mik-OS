use mik_os_x86::{build_image, disk_image, find_qemu, kernel_elf, workspace_root};
use std::env;
use std::process::Command;

fn main() {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("build") => build_kernel(),
        Some("image") => {
            build_kernel();
            write_image();
        }
        // `qemu` boots the real BIOS path: boot sector -> stage2 -> kernel.
        Some("qemu") | Some("run") => run_disk(),
        // `pvh` keeps the QEMU -kernel direct-boot path for comparison.
        Some("pvh") => run_pvh(),
        _ => {
            eprintln!("usage: mik-os-x86 <build|image|qemu|pvh>");
            std::process::exit(1);
        }
    }
}

fn cargo() -> String {
    env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
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

fn write_image() -> std::path::PathBuf {
    let elf = kernel_elf();
    let img = disk_image();
    let elf_bytes = std::fs::read(&elf).expect("kernel ELF not built");
    let image = build_image(&elf_bytes).expect("failed to build disk image");
    std::fs::write(&img, &image).expect("failed to write disk image");
    println!("wrote {} ({} bytes)", img.display(), image.len());
    img
}

fn run_disk() {
    build_kernel();
    let img = write_image();
    let qemu = find_qemu();
    let mut cmd = Command::new(qemu);
    cmd.arg("-drive")
        .arg(format!("format=raw,file={}", img.display()))
        .arg("-serial")
        .arg("stdio")
        .arg("-display")
        .arg("none")
        .arg("-no-reboot")
        .arg("-no-shutdown");
    let status = cmd.status().expect("failed to run qemu");
    std::process::exit(status.code().unwrap_or(1));
}

fn run_pvh() {
    build_kernel();
    let img = write_image();
    let elf = kernel_elf();
    let qemu = find_qemu();
    let mut cmd = Command::new(qemu);
    cmd.arg("-kernel")
        .arg(elf)
        // Attach the same disk so the ATA driver and Mik-FS work here too.
        .arg("-drive")
        .arg(format!("format=raw,file={}", img.display()))
        .arg("-serial")
        .arg("stdio")
        .arg("-display")
        .arg("none")
        .arg("-no-reboot")
        .arg("-no-shutdown");
    let status = cmd.status().expect("failed to run qemu");
    std::process::exit(status.code().unwrap_or(1));
}
