//! Minimal IDT: interrupt-gate entries for the 32 CPU exception vectors,
//! each pointing at an `isr_N` stub from `isr.S`.

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct IdtEntry {
    offset_lo: u16,
    selector: u16,
    ist: u8,
    flags: u8,
    offset_mid: u16,
    offset_hi: u32,
    _reserved: u32,
}

impl IdtEntry {
    const fn new(handler: u64) -> Self {
        Self {
            offset_lo: handler as u16,
            selector: 0x08,      // gdt64 code segment from boot.S
            ist: 0,
            flags: 0x8E,         // present, DPL 0, 64-bit interrupt gate
            offset_mid: (handler >> 16) as u16,
            offset_hi: (handler >> 32) as u32,
            _reserved: 0,
        }
    }
}

#[repr(C, packed)]
struct IdtPtr {
    limit: u16,
    base: u64,
}

extern "C" {
    static isr_table: [u64; 32];
}

static mut IDT: [IdtEntry; 32] = [IdtEntry::new(0); 32];

/// Fill the IDT from `isr_table` and load it.
///
/// Safety: must run before any exception or interrupt can fire; intended for
/// one-shot kernel init.
pub unsafe fn init() {
    let idt = core::ptr::addr_of_mut!(IDT);
    for (i, entry) in (*idt).iter_mut().enumerate() {
        *entry = IdtEntry::new(isr_table[i]);
    }
    let ptr = IdtPtr {
        limit: (core::mem::size_of_val(&*idt) - 1) as u16,
        base: idt as u64,
    };
    core::arch::asm!("lidt [{}]", in(reg) &ptr, options(readonly, nostack));
}
