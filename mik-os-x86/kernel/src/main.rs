#![no_std]
#![no_main]

use core::arch::global_asm;

global_asm!(include_str!("boot.S"));
global_asm!(include_str!("boot16.S"));
global_asm!(include_str!("stage2.S"));
global_asm!(include_str!("isr.S"));
global_asm!(include_str!("user.S"));

extern "C" {
    static user_prog_start: u8;
    static user_prog_end: u8;
}

mod e820;
mod idt;
mod mem;
mod pic;
mod sched;
mod seg;
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

        // Ring-3 plumbing: user segments + TSS, PIC remap, PIT timer. Nothing
        // fires yet — IF stays clear until the first iretq into user mode.
        seg::init();
        pic::init_pic();
        pic::init_pit();
        serial::write_str("gdt/tss/pic/pit ok\n");

        // First user address space: a private PML4 sharing the kernel's
        // identity PD, with the user blob mapped at 0x40000000 — a VA that is
        // unmapped under the kernel's own PML4, so running it proves the
        // second address space is real. Executed from ring 0 for now; ring 3
        // entry is the M2.3 scheduler milestone.
        const USER_VA: u64 = 0x4000_0000;
        let frame = mem::alloc_frame().expect("out of frames for user page");
        let len = &user_prog_end as *const u8 as usize
            - &user_prog_start as *const u8 as usize;
        core::ptr::copy_nonoverlapping(
            &user_prog_start as *const u8,
            frame as *mut u8,
            len,
        );
        let up4 = mem::build_user_table();
        mem::map_4k(up4 as *mut u64, USER_VA, frame, mem::PTE_P | mem::PTE_W | mem::PTE_U);
        let kcr3 = mem::kernel_pml4();
        serial::write_str("user page -> ");
        mem::switch_cr3(up4);
        core::arch::asm!("call {}", in(reg) USER_VA); // prints 'U'
        mem::switch_cr3(kcr3);
        serial::write_str(" <- ran under second CR3\n");
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
