//! PCI bus enumeration via legacy config-space ports 0xCF8/0xCFC — the
//! probe every real driver starts from. Prints each present function as
//! "pci bb.dd.f vend:dev class.sub" on COM1. Only enumeration lives here;
//! device drivers (storage, NIC) claim these functions in later milestones.

use crate::serial::{self};

const ADDR: u16 = 0xCF8;
const DATA: u16 = 0xCFC;

unsafe fn outl(port: u16, v: u32) {
    core::arch::asm!("out dx, eax", in("dx") port, in("eax") v);
}

unsafe fn inl(port: u16) -> u32 {
    let v: u32;
    core::arch::asm!("in eax, dx", in("dx") port, out("eax") v);
    v
}

/// Read config dword at bus/device/function/register (reg is dword-indexed).
fn cfg_read(bus: u8, dev: u8, func: u8, reg: u8) -> u32 {
    let addr = 0x8000_0000u32
        | (bus as u32) << 16
        | (dev as u32) << 11
        | (func as u32) << 8
        | ((reg as u32) << 2);
    unsafe {
        outl(ADDR, addr);
        inl(DATA)
    }
}

/// Enumerate all 256 buses x 32 devices x 8 functions and print what
/// responds. Full-bus scanning is the simple correct answer for a fixed
/// QEMU topology — ~65k port reads at boot, under a second.
pub fn scan() {
    serial::write_str("pci scan:\n");
    for bus in 0..=255u8 {
        for dev in 0..32u8 {
            let id = cfg_read(bus, dev, 0, 0);
            if id as u16 == 0xFFFF {
                continue;
            }
            let funcs = if cfg_read(bus, dev, 0, 3) & 0x0080_0000 != 0 {
                8 // multifunction bit in header type
            } else {
                1
            };
            for func in 0..funcs {
                let id = cfg_read(bus, dev, func, 0);
                let vendor = id as u16;
                if vendor == 0xFFFF {
                    continue;
                }
                let class = cfg_read(bus, dev, func, 2) >> 16; // class.subclass
                serial::write_str("  pci ");
                serial::write_hexn(bus as u64, 2);
                serial::write_str(".");
                serial::write_hexn(dev as u64, 2);
                serial::write_str(".");
                serial::write_hexn(func as u64, 2);
                serial::write_str(" ");
                serial::write_hexn(vendor as u64, 4);
                serial::write_str(":");
                serial::write_hexn((id >> 16) as u64, 4);
                serial::write_str(" class ");
                serial::write_hexn(class as u64, 4);
                serial::write_str("\n");
            }
        }
    }
}
