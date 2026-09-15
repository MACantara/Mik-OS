//! 8259 PIC remap + PIT channel 0 — the classic pre-APIC timer path.
//!
//! The PICs default to vectors 0-15, colliding with CPU exceptions; remapping
//! puts IRQ0 at vector 32 and the slave at 40. The PIT's channel 0 output is
//! wired to IRQ0, so a ~100 Hz square wave drives the scheduler tick.

use crate::serial::{inb, outb};

const PIC1_CMD: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_CMD: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;
const PIT_CMD: u16 = 0x43;
const PIT_CH0: u16 = 0x40;

/// End-of-interrupt to the master PIC (IRQ0-7 live there).
pub unsafe fn eoi() {
    outb(PIC1_CMD, 0x20);
}

/// Remap both PICs to vectors 32/40, then mask everything except IRQ0.
/// The sequence is ICW1-4 on each chip; a few `inb`s give the chips time.
pub unsafe fn init_pic() {
    outb(PIC1_CMD, 0x11); // ICW1: edge-triggered, cascade, ICW4 needed
    outb(PIC2_CMD, 0x11);
    outb(PIC1_DATA, 0x20); // ICW2: IRQ0-7 -> vectors 32-39
    outb(PIC2_DATA, 0x28); // ICW2: IRQ8-15 -> vectors 40-47
    outb(PIC1_DATA, 0x04); // ICW3: slave on IRQ2
    outb(PIC2_DATA, 0x02); // ICW3: slave identity
    outb(PIC1_DATA, 0x01); // ICW4: 8086 mode
    outb(PIC2_DATA, 0x01);
    let _ = inb(PIC1_DATA); // settle
    outb(PIC1_DATA, 0xE8); // unmask IRQ0 (timer), IRQ1 (kbd), IRQ4 (uart)
    outb(PIC2_DATA, 0xFF); // mask all slave lines
}

/// PIT channel 0, square-wave mode, ~100 Hz (1193182 / 11932).
pub unsafe fn init_pit() {
    const DIVISOR: u16 = 11932;
    outb(PIT_CMD, 0x36); // ch0, lobyte/hibyte, mode 3
    outb(PIT_CH0, (DIVISOR & 0xFF) as u8);
    outb(PIT_CH0, (DIVISOR >> 8) as u8);
}
