//! Port I/O primitives + COM1 serial output — the kernel's only console.
//! Assumes QEMU's UART transmitter is always ready; a real UART would poll
//! LSR bit 5 (THRE) before each byte.

const COM1: u16 = 0x3F8;

pub unsafe fn outb(port: u16, v: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") v);
}

pub unsafe fn inb(port: u16) -> u8 {
    let v: u8;
    core::arch::asm!("in al, dx", in("dx") port, out("al") v);
    v
}

pub fn write_byte(b: u8) {
    unsafe { outb(COM1, b) }
}

/// Enable the UART's receive-data interrupt (IER bit 0 -> IRQ4) and its
/// 16-byte FIFOs (FCR: enable + clear RX/TX). The FIFO matters on emulated
/// UARTs too: without it the device accepts one byte per host write and
/// QEMU may hold a burst host-side indefinitely.
pub unsafe fn enable_rx_irq() {
    outb(COM1 + 2, 0x01); // FCR: FIFOs on (no clear — keep early input)
    outb(COM1 + 1, inb(COM1 + 1) | 0x01);
}

/// Drain every byte the UART's receiver is holding into the input buffer.
/// Called from the IRQ4 handler and, as a delivery safety net, from the
/// timer tick — an edge-triggered IRQ can be missed if a byte arrives
/// inside an IF=0 window that has already drained it.
pub unsafe fn drain_rx() {
    while inb(COM1 + 5) & 1 != 0 {
        crate::input::push(inb(COM1));
        crate::sched::wake_on_input();
    }
}

/// IRQ4: new input arrived — drain and EOI. Never reschedules; the woken
/// reader is picked up by the next tick.
#[no_mangle]
extern "C" fn uart_handler(frame: *mut crate::sched::IrqFrame) -> *mut crate::sched::IrqFrame {
    unsafe {
        drain_rx();
        crate::pic::eoi();
    }
    frame
}

pub fn write_str(s: &str) {
    for &b in s.as_bytes() {
        write_byte(b);
    }
}

pub fn write_hex(v: u64) {
    write_str("0x");
    write_hexn(v, 16);
}

/// Low `digits` nibbles of `v` in hex — for bus/dev/func-style fields where
/// the full 64-bit padding is noise.
pub fn write_hexn(v: u64, digits: u32) {
    for i in (0..digits).rev() {
        let nibble = ((v >> (i * 4)) & 0xF) as u8;
        write_byte(if nibble < 10 { b'0' + nibble } else { b'A' + nibble - 10 });
    }
}

pub fn write_dec(mut v: u64) {
    let mut buf = [0u8; 20];
    let mut i = 20;
    loop {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    write_str(unsafe { core::str::from_utf8_unchecked(&buf[i..]) });
}
