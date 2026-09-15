//! IDT: interrupt-gate entries for the 32 CPU exception vectors, the timer
//! IRQ (vector 32), and the `int 0x80` syscall gate (vector 128, DPL=3).

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
    const fn new(handler: u64, flags: u8) -> Self {
        Self {
            offset_lo: handler as u16,
            selector: 0x08,      // kernel code segment
            ist: 0,
            flags,
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
    fn isr_timer();
    fn isr_syscall();
}

const GATE_INT: u8 = 0x8E; // present, DPL 0, 64-bit interrupt gate
const GATE_INT_USER: u8 = 0xEE; // present, DPL 3, 64-bit interrupt gate
const TIMER_VEC: usize = 32; // IRQ0 after PIC remap
const SYSCALL_VEC: usize = 0x80;

static mut IDT: [IdtEntry; 256] = [IdtEntry::new(0, 0); 256];

/// Fill the IDT: exception stubs, timer IRQ, and the user-callable syscall
/// gate, then `lidt`.
///
/// Safety: must run before exceptions, interrupts, or `int 0x80` can fire;
/// intended for one-shot kernel init.
pub unsafe fn init() {
    let idt = core::ptr::addr_of_mut!(IDT);
    for (i, entry) in (*idt).iter_mut().enumerate().take(32) {
        *entry = IdtEntry::new(isr_table[i], GATE_INT);
    }
    (*idt)[TIMER_VEC] = IdtEntry::new(isr_timer as *const () as u64, GATE_INT);
    (*idt)[SYSCALL_VEC] = IdtEntry::new(isr_syscall as *const () as u64, GATE_INT_USER);
    let ptr = IdtPtr {
        limit: (core::mem::size_of_val(&*idt) - 1) as u16,
        base: idt as u64,
    };
    core::arch::asm!("lidt [{}]", in(reg) &ptr, options(readonly, nostack));
}
