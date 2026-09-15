#![no_std]
#![no_main]

use core::arch::global_asm;

global_asm!(include_str!("boot.S"));
global_asm!(include_str!("boot16.S"));
global_asm!(include_str!("stage2.S"));
global_asm!(include_str!("isr.S"));
global_asm!(include_str!("user.S"));

extern "C" {
    static prog_a_start: u8;
    static prog_a_end: u8;
    static prog_b_start: u8;
    static prog_b_end: u8;
    static prog_sh_start: u8;
    static prog_sh_end: u8;
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
        serial::write_str("gdt/tss/pic ok\n");

        // Spawn the user processes and hand the CPU to the scheduler. The
        // deterministic sequence is "ADcEDp" (demand fault, COW child, exec,
        // parent's isolated read, parent's own write) with B's timer-driven
        // 'B's and the shell's "mik> " prompt interleaved anywhere; after
        // the demo only B and the shell remain, waiting on ticks and input.
        let prog_a = core::slice::from_raw_parts(
            &prog_a_start as *const u8,
            &prog_a_end as *const u8 as usize - &prog_a_start as *const u8 as usize,
        );
        let prog_b = core::slice::from_raw_parts(
            &prog_b_start as *const u8,
            &prog_b_end as *const u8 as usize - &prog_b_start as *const u8 as usize,
        );
        let prog_sh = core::slice::from_raw_parts(
            &prog_sh_start as *const u8,
            &prog_sh_end as *const u8 as usize - &prog_sh_start as *const u8 as usize,
        );
        sched::spawn(prog_a);
        sched::spawn(prog_b);
        sched::spawn(prog_sh);
        serial::write_str("sched: 3 procs\n");
        // Arm the tick last: a pending IRQ0 would be delivered the instant the
        // first iretq sets IF and would preempt before proc A ever runs.
        pic::init_pit();
        sched::start();
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}
