#![no_std]
#![no_main]

use core::arch::global_asm;

global_asm!(include_str!("boot.S"));

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    const COM1: u16 = 0x3f8;
    let banner = b"Mik-64 -> x86-64 long mode\n";
    unsafe {
        for b in banner {
            core::arch::asm!("out dx, al", in("dx") COM1, in("al") *b);
        }
    }
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}
