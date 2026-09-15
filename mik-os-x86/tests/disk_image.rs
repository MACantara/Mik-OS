use mik_os_x86::{build_image, kernel_elf};
use std::process::Command;

fn kernel_elf_bytes() -> Vec<u8> {
    let status = Command::new("cargo")
        .args(["build", "-p", "mik-os-x86-kernel", "--target", "x86_64-unknown-none"])
        .status()
        .expect("cargo build");
    assert!(status.success(), "mik-os-x86-kernel failed to build");
    std::fs::read(kernel_elf()).expect("kernel ELF not built")
}

#[test]
fn image_layout_is_bootable() {
    let img = build_image(&kernel_elf_bytes()).expect("build_image");

    assert_eq!(img.len(), 64 * 1024, "image must be 64 KiB");
    assert_eq!(img[0], 0xFA, "boot sector must start with cli");
    assert_eq!(&img[510..512], &[0x55, 0xAA], "missing BIOS signature");

    let klen = u32::from_le_bytes(img[0x1F8..0x1FC].try_into().unwrap());
    assert!(klen > 0, "kernel_len was not patched into the boot sector");
}
