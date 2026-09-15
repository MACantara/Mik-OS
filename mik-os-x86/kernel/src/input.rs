//! Shared input ring buffer: device ISRs (keyboard IRQ1, UART IRQ4) push
//! bytes, `sys_read` pops them. No locking is needed — producers run inside
//! interrupt gates and the consumer inside the `int 0x80` gate, all with
//! IF=0, so pushes and pops can never interleave on this single CPU.

const CAP: usize = 64;
static mut BUF: [u8; CAP] = [0; CAP];
static mut HEAD: usize = 0; // next pop
static mut LEN: usize = 0;

/// Called from device ISRs. Drops the byte when full — a 64-byte backlog of
/// untyped input is beyond this shell's needs.
pub fn push(b: u8) {
    unsafe {
        let len = *core::ptr::addr_of!(LEN);
        if len < CAP {
            let head = *core::ptr::addr_of!(HEAD);
            (*core::ptr::addr_of_mut!(BUF))[(head + len) % CAP] = b;
            *core::ptr::addr_of_mut!(LEN) = len + 1;
        }
    }
}

/// Called from `sys_read`. `None` means empty — the caller blocks instead
/// of spinning.
pub fn pop() -> Option<u8> {
    unsafe {
        let len = *core::ptr::addr_of!(LEN);
        if len == 0 {
            return None;
        }
        let head = *core::ptr::addr_of!(HEAD);
        let b = (*core::ptr::addr_of!(BUF))[head];
        *core::ptr::addr_of_mut!(HEAD) = (head + 1) % CAP;
        *core::ptr::addr_of_mut!(LEN) = len - 1;
        Some(b)
    }
}
