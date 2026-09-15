#![no_std]
#![no_main]

use core::arch::global_asm;

global_asm!(include_str!("boot.S"));
global_asm!(include_str!("boot16.S"));
global_asm!(include_str!("stage2.S"));
global_asm!(include_str!("isr.S"));

mod e820;
mod idt;
mod serial;

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    serial::write_str("Mik-64 -> x86-64 long mode\n");
    unsafe {
        idt::init();
    }

    let mut map = [e820::E820Entry { base: 0, len: 0, typ: 0, acpi: 0 }; 32];
    let n = e820::read_map(&mut map);
    serial::write_str("e820 entries=");
    serial::write_dec(n as u64);
    let mut usable = 0u64;
    for e in &map[..n] {
        if e.usable() {
            usable += e.len;
        }
    }
    serial::write_str(" usable=");
    serial::write_dec(usable / 1024);
    serial::write_str("K\n");

    unsafe {
        // Prove the IDT works: int3 delivers vector 3 to the stub, which
        // prints "EX03" on COM1 and halts. Runs last — it never returns.
        core::arch::asm!("int3");
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
