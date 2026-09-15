#![no_std]
#![no_main]

use core::arch::global_asm;

global_asm!(include_str!("boot.S"));
global_asm!(include_str!("boot16.S"));
global_asm!(include_str!("stage2.S"));
global_asm!(include_str!("isr.S"));

mod e820;
mod idt;
mod mem;
mod serial;

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    serial::write_str("Mik-64 -> x86-64 long mode\n");
    unsafe {
        idt::init();

        // Grow the identity map first: the free list writes a link into every
        // frame it hands out, so all usable RAM must be mapped before init —
        // and the .bss boot tables are already reachable under the old map.
        mem::extend_identity_map();
        serial::write_str("identity map: 1 GiB\n");

        let frames = mem::init();
        serial::write_str("usable frames=");
        serial::write_dec(frames);
        serial::write_str("\n");

        // Free-list sanity: alloc a,b; free a; alloc c must reuse a (LIFO).
        let a = mem::alloc_frame().expect("out of frames");
        let _b = mem::alloc_frame().expect("out of frames");
        mem::free_frame(a);
        let c = mem::alloc_frame().expect("out of frames");
        serial::write_str(if c == a { "alloc/free ok\n" } else { "alloc/free BAD\n" });
    }

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
