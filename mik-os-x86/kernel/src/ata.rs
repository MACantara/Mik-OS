//! ATA PIO driver for the primary-bus master — the same disk that booted
//! us. QEMU's default `-drive` lands on the PIIX3 IDE controller (the
//! `class 0101` entry in the PCI scan) in legacy mode, so the fixed ports
//! 0x1F0..0x1F7 / 0x3F6 need no BAR setup.
//!
//! LBA28, one sector at a time, polled: no IRQ14, no DMA. Writes end with
//! FLUSH CACHE so data reaches the host image file — that is what makes
//! files survive a QEMU restart.

use crate::serial::{inb, outb};

const DATA: u16 = 0x1F0;
const COUNT: u16 = 0x1F2;
const LBA_LO: u16 = 0x1F3;
const LBA_MID: u16 = 0x1F4;
const LBA_HI: u16 = 0x1F5;
const DRIVE: u16 = 0x1F6;
const STATUS: u16 = 0x1F7; // read
const CMD: u16 = 0x1F7; // write
const ALT: u16 = 0x3F6;

const BSY: u8 = 0x80;
const DRQ: u8 = 0x08;
const ERR: u8 = 0x01;

static mut PRESENT: bool = false;

/// Wait for BSY to clear. The four ALT reads first give the drive the
/// ~400ns it needs after a drive select before its status is meaningful.
unsafe fn wait_ready() -> bool {
    for _ in 0..4 {
        inb(ALT);
    }
    let mut spins = 0u32;
    while inb(STATUS) & BSY != 0 {
        spins += 1;
        if spins > 10_000_000 {
            return false;
        }
    }
    true
}

/// Wait for DRQ (data ready) or ERR.
unsafe fn wait_drq() -> bool {
    let mut spins = 0u32;
    loop {
        let s = inb(STATUS);
        if s & ERR != 0 {
            return false;
        }
        if s & DRQ != 0 {
            return true;
        }
        spins += 1;
        if spins > 10_000_000 {
            return false;
        }
    }
}

unsafe fn select(lba: u32) {
    outb(DRIVE, 0xE0 | ((lba >> 24) & 0xF) as u8); // master, LBA mode
    outb(COUNT, 1);
    outb(LBA_LO, lba as u8);
    outb(LBA_MID, (lba >> 8) as u8);
    outb(LBA_HI, (lba >> 16) as u8);
}

/// Read one 512-byte sector into `buf`. False on timeout or drive error.
pub unsafe fn read_sector(lba: u32, buf: *mut u8) -> bool {
    if !wait_ready() {
        return false;
    }
    select(lba);
    outb(CMD, 0x20); // READ SECTORS
    if !wait_drq() {
        return false;
    }
    // rep insw: 256 words from port DX into [rdi]. DF is clear per the ABI.
    core::arch::asm!(
        "rep insw",
        in("dx") DATA,
        in("rdi") buf,
        in("rcx") 256u32,
    );
    true
}

/// Write one 512-byte sector from `buf`, then FLUSH CACHE so the write is
/// durable before we return.
pub unsafe fn write_sector(lba: u32, buf: *const u8) -> bool {
    if !wait_ready() {
        return false;
    }
    select(lba);
    outb(CMD, 0x30); // WRITE SECTORS
    if !wait_drq() {
        return false;
    }
    core::arch::asm!(
        "rep outsw",
        in("dx") DATA,
        in("rsi") buf,
        in("rcx") 256u32,
    );
    if !wait_ready() {
        return false;
    }
    outb(CMD, 0xE7); // FLUSH CACHE
    wait_ready()
}

/// Probe the drive: read sector 0 and check the boot signature. False on
/// the PVH path, which attaches no disk — every caller must then degrade
/// gracefully rather than hang a nonexistent device.
pub unsafe fn init() -> bool {
    let mut sector = [0u8; 512];
    let ok = read_sector(0, sector.as_mut_ptr())
        && sector[510] == 0x55
        && sector[511] == 0xAA;
    *core::ptr::addr_of_mut!(PRESENT) = ok;
    ok
}

pub fn present() -> bool {
    unsafe { *core::ptr::addr_of!(PRESENT) }
}
