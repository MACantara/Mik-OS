//! VGA text-mode output: the user-visible console. `sys_write` mirrors
//! every byte here and to COM1 — kernel diagnostics (`serial::write_str`)
//! stay serial-only. 80x25 cells at phys 0xB8000, attribute 0x07 (light
//! gray on black), with newline, backspace, and bottom-line scroll.

const VGA: *mut u8 = 0xB8000 as *mut u8;
const COLS: usize = 80;
const ROWS: usize = 25;
const ATTR: u8 = 0x07;

static mut CURSOR: usize = 0; // cell index, 0..COLS*ROWS

pub fn put_byte(b: u8) {
    unsafe {
        let cur = core::ptr::addr_of_mut!(CURSOR);
        match b {
            b'\n' => *cur = (*cur / COLS + 1) * COLS,
            b'\r' => *cur = (*cur / COLS) * COLS,
            0x08 => {
                if *cur % COLS > 0 {
                    *cur -= 1;
                    VGA.add(*cur * 2).write_volatile(b' ');
                }
            }
            _ if (0x20..0x7F).contains(&b) => {
                VGA.add(*cur * 2).write_volatile(b);
                VGA.add(*cur * 2 + 1).write_volatile(ATTR);
                *cur += 1;
            }
            _ => {}
        }
        while *cur >= COLS * ROWS {
            scroll();
            *cur -= COLS;
        }
    }
}

/// Move rows 1..24 up one line and clear the bottom row.
fn scroll() {
    unsafe {
        core::ptr::copy(
            VGA.add(COLS * 2),
            VGA,
            COLS * (ROWS - 1) * 2,
        );
        for c in 0..COLS {
            let cell = (ROWS - 1) * COLS + c;
            VGA.add(cell * 2).write_volatile(b' ');
            VGA.add(cell * 2 + 1).write_volatile(ATTR);
        }
    }
}
