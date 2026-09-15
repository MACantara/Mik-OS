//! PS/2 keyboard on IRQ1 (vector 33): read the scancode from port 0x60,
//! decode set-1 make codes to ASCII, push into the shared input buffer,
//! and wake anyone blocked in `sys_read`. Break codes (bit 7) release keys;
//! only shift is tracked, everything else is ignored.

use crate::input;
use crate::pic;
use crate::sched::{self, IrqFrame};
use crate::serial::inb;

const KBD_DATA: u16 = 0x60;

// Set-1 make codes 0x00..0x3A -> ASCII (0 = no mapping): Esc, digits row,
// qwerty row, asdf row, zxcv row, then keypad-* / alt / space / caps.
const UNSHIFTED: [u8; 0x3B] =
    *b"\x00\x001234567890-=\x08\tqwertyuiop[]\n\x00asdfghjkl;'`\x00\\zxcvbnm,./\x00*\x00 \x00";
const SHIFTED: [u8; 0x3B] =
    *b"\x00\x00!@#$%^&*()_+\x08\tQWERTYUIOP{}\n\x00ASDFGHJKL:\"~\x00|ZXCVBNM<>?\x00*\x00 \x00";

static mut SHIFT: bool = false;

fn decode(sc: u8) -> Option<u8> {
    unsafe {
        let shift = core::ptr::addr_of_mut!(SHIFT);
        match sc {
            0x2A | 0x36 => *shift = true,          // left/right shift make
            0xAA | 0xB6 => *shift = false,         // shift break
            _ if sc & 0x80 != 0 => {}              // other break codes
            _ if (sc as usize) < UNSHIFTED.len() => {
                let b = if *shift { SHIFTED } else { UNSHIFTED }[sc as usize];
                if b != 0 {
                    return Some(b);
                }
            }
            _ => {}
        }
        None
    }
}

/// IRQ1: decode whatever the keyboard sent, buffer it, EOI. Never
/// reschedules — the woken reader is picked up by the next tick.
#[no_mangle]
extern "C" fn kbd_handler(frame: *mut IrqFrame) -> *mut IrqFrame {
    unsafe {
        if let Some(b) = decode(inb(KBD_DATA)) {
            input::push(b);
            sched::wake_on_input();
        }
        pic::eoi();
    }
    frame
}
