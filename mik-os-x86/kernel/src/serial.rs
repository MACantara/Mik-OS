//! COM1 serial output — the kernel's only console. Assumes QEMU's UART
//! transmitter is always ready; a real UART would poll LSR bit 5 (THRE)
//! before each byte.

const COM1: u16 = 0x3F8;

pub fn write_byte(b: u8) {
    unsafe { core::arch::asm!("out dx, al", in("dx") COM1, in("al") b) }
}

pub fn write_str(s: &str) {
    for &b in s.as_bytes() {
        write_byte(b);
    }
}

pub fn write_hex(v: u64) {
    write_str("0x");
    for i in (0..16).rev() {
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
