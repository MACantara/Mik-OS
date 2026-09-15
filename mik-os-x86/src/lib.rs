//! Build a raw disk image from the kernel ELF for the BIOS boot path.
//!
//! The kernel ELF contains three extra payload sections beyond the usual
//! kernel image: `.boot16` (512-byte boot sector, VMA 0x7C00), `.stage2`
//! (32-bit loader, VMA 0x7E00), and the kernel proper (PT_LOADs at 0x400000+).
//! `build_image` extracts them and writes a 64 KiB raw image:
//!
//! ```text
//! sector 0        : .boot16 (kernel_len patched in at offset 0x1F8)
//! sectors 1..N    : .stage2, then the flat kernel image
//! ```

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

const KERNEL_VMA: u64 = 0x400000;
const IMAGE_SIZE: usize = 64 * 1024; // 128 sectors: boot + 127 loaded by INT 13h
const KLEN_OFFSET: usize = 0x1F8; // patched u32 inside .boot16

fn r16(b: &[u8], off: usize) -> usize {
    u16::from_le_bytes(b[off..off + 2].try_into().unwrap()) as usize
}
fn r32(b: &[u8], off: usize) -> usize {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap()) as usize
}
fn r64(b: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(b[off..off + 8].try_into().unwrap())
}

/// Extract a named section's file bytes from an ELF64 image.
pub fn elf_section(elf: &[u8], name: &str) -> Result<Vec<u8>, String> {
    if elf.len() < 64 || &elf[0..4] != b"\x7fELF" || elf[4] != 2 || elf[5] != 1 {
        return Err("not an ELF64-LE image".into());
    }
    let shoff = r64(elf, 0x28) as usize;
    let shentsize = r16(elf, 0x3A);
    let shnum = r16(elf, 0x3C);
    let shstrndx = r16(elf, 0x3E);
    let sh = |i: usize| -> &[u8] {
        &elf[shoff + i * shentsize..shoff + (i + 1) * shentsize]
    };
    // sh_name offsets live in the section-header string table.
    let strtab = sh(shstrndx);
    let strs = &elf[r64(strtab, 0x18) as usize..];
    let strname = |off: usize| -> &str {
        let end = strs[off..].iter().position(|&c| c == 0).unwrap_or(0);
        std::str::from_utf8(&strs[off..off + end]).unwrap_or("")
    };
    for i in 0..shnum {
        let s = sh(i);
        if strname(r32(s, 0)) == name {
            let (off, size) = (r64(s, 0x18) as usize, r64(s, 0x20) as usize);
            return Ok(elf[off..off + size].to_vec());
        }
    }
    Err(format!("section {name} not found"))
}

/// Flatten the kernel image: every allocated section with file content at or
/// above `KERNEL_VMA`, placed at `sh_addr - KERNEL_VMA`. `.bss` (SHT_NOBITS)
/// carries no file bytes — stage2 zeroes it at load time.
pub fn elf_kernel_flat(elf: &[u8]) -> Result<Vec<u8>, String> {
    if elf.len() < 64 || &elf[0..4] != b"\x7fELF" || elf[4] != 2 || elf[5] != 1 {
        return Err("not an ELF64-LE image".into());
    }
    let shoff = r64(elf, 0x28) as usize;
    let shentsize = r16(elf, 0x3A);
    let shnum = r16(elf, 0x3C);
    const SHF_ALLOC: u64 = 0x2;
    const SHT_NOBITS: u64 = 8;
    let mut flat = Vec::new();
    for i in 0..shnum {
        let s = &elf[shoff + i * shentsize..shoff + (i + 1) * shentsize];
        let typ = r32(s, 0x04) as u64;
        let (flags, addr) = (r64(s, 0x08), r64(s, 0x10));
        if flags & SHF_ALLOC == 0 || typ == SHT_NOBITS || addr < KERNEL_VMA {
            continue;
        }
        let (off, size) = (r64(s, 0x18) as usize, r64(s, 0x20) as usize);
        let at = (addr - KERNEL_VMA) as usize;
        if flat.len() < at + size {
            flat.resize(at + size, 0);
        }
        flat[at..at + size].copy_from_slice(&elf[off..off + size]);
    }
    if flat.is_empty() {
        return Err("no kernel sections found".into());
    }
    Ok(flat)
}

/// Assemble the raw boot disk image from the kernel ELF.
pub fn build_image(elf: &[u8]) -> Result<Vec<u8>, String> {
    let mut boot = elf_section(elf, ".boot16")?;
    let stage2 = elf_section(elf, ".stage2")?;
    let kernel = elf_kernel_flat(elf)?;
    if boot.len() != 512 {
        return Err(format!(".boot16 is {} bytes, must be 512", boot.len()));
    }
    if boot[510..512] != [0x55, 0xAA] {
        return Err(".boot16 is missing the 0xAA55 signature".into());
    }
    if stage2.is_empty() {
        return Err(".stage2 is empty".into());
    }
    if kernel.len() as u64 + 512 + stage2.len() as u64 > IMAGE_SIZE as u64 {
        return Err("kernel does not fit in the 64 KiB image".into());
    }
    boot[KLEN_OFFSET..KLEN_OFFSET + 4]
        .copy_from_slice(&(kernel.len() as u32).to_le_bytes());

    let mut img = Vec::with_capacity(IMAGE_SIZE);
    img.extend_from_slice(&boot);
    img.extend_from_slice(&stage2);
    img.extend_from_slice(&kernel);
    img.resize(IMAGE_SIZE, 0);
    Ok(img)
}

pub fn workspace_root() -> PathBuf {
    PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("manifest has parent")
        .to_path_buf()
}

pub fn kernel_elf() -> PathBuf {
    workspace_root()
        .join("target")
        .join("x86_64-unknown-none")
        .join("debug")
        .join("mik-os-x86-kernel")
}

pub fn disk_image() -> PathBuf {
    workspace_root().join("target").join("mik-os-x86.img")
}

pub fn find_qemu() -> PathBuf {
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
    for c in [
        r"C:\Program Files\qemu\qemu-system-x86_64.exe",
        r"C:\Program Files (x86)\qemu\qemu-system-x86_64.exe",
    ] {
        if Path::new(c).exists() {
            return PathBuf::from(c);
        }
    }
    panic!("qemu-system-x86_64 not found; set QEMU or add it to PATH");
}
