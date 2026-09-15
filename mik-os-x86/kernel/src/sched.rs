//! Process table + round-robin scheduler for x86-64.
//!
//! A process is: an address space (PML4 phys), a kernel stack, and a saved
//! `IrqFrame`. All context arrives through the same shape — a timer IRQ or an
//! `int 0x80` pushes the CPU frame, `isr_*` pushes the 15 GPRs, and the Rust
//! handler returns *which frame to resume*. Returning a different process's
//! frame is the whole context switch.

use crate::mem;
use crate::pic;
use crate::seg;
use crate::serial;

/// The full frame saved on a process's kernel stack: the 15 GPRs pushed by
/// `SAVE_REGS` (r15 at the lowest address) followed by the CPU-pushed iret
/// frame (rip, cs, rflags, rsp, ss).
#[repr(C)]
pub struct IrqFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

const EMPTY: u8 = 0;
const READY: u8 = 1;
const DEAD: u8 = 2;

pub struct Proc {
    pml4: u64,
    kstack_top: u64,
    frame: *mut IrqFrame,
    state: u8,
    /// Lazily-mapped heap ceiling: [USER_DATA_VA, brk) is demand-paged.
    brk: u64,
}

const NPROC: usize = 3;
static mut PROCS: [Proc; NPROC] = [
    Proc { pml4: 0, kstack_top: 0, frame: core::ptr::null_mut(), state: EMPTY, brk: 0 },
    Proc { pml4: 0, kstack_top: 0, frame: core::ptr::null_mut(), state: EMPTY, brk: 0 },
    Proc { pml4: 0, kstack_top: 0, frame: core::ptr::null_mut(), state: EMPTY, brk: 0 },
];
static mut CUR: usize = 0;
/// Gates timer work: ticks arriving before start() (e.g. the IRQ0 the BIOS
/// PIT leaves latched from boot) are EOI'd and dropped, not scheduled.
static mut SCHED_ACTIVE: bool = false;

const USER_CODE_VA: u64 = 0x4000_0000;
const USER_STACK_VA: u64 = 0x4000_1000; // one page; rsp starts at its top
const USER_DATA_VA: u64 = 0x4000_2000;  // heap base; [base, brk) demand-paged
const BRK_MAX: u64 = USER_DATA_VA + 0x1_0000; // 64 KiB cap on sbrk growth
const USER_REGION_END: u64 = 0x8000_0000; // end of the private user PD span

extern "C" {
    fn enter_user(frame: *mut IrqFrame) -> !;
    static prog_c_start: u8;
    static prog_c_end: u8;
}

/// Create a process running `code` in a fresh address space.
///
/// The user gets a code page at `0x40000000` and a one-page stack at
/// `0x40001000`, both `PTE_U`. The kernel stack is a single frame — enough
/// for one saved frame plus shallow handler calls. A fabricated `IrqFrame`
/// at the stack top is what `irq_tail` pops on first entry.
pub unsafe fn spawn(code: &[u8]) {
    let procs = &mut *core::ptr::addr_of_mut!(PROCS);
    let i = procs.iter().position(|p| p.state == EMPTY).expect("no free slot");
    let pml4 = mem::build_user_table();

    let code_frame = mem::alloc_frame().expect("out of frames");
    core::ptr::copy_nonoverlapping(code.as_ptr(), code_frame as *mut u8, code.len());
    mem::map_4k(
        pml4 as *mut u64,
        USER_CODE_VA,
        code_frame,
        mem::PTE_P | mem::PTE_W | mem::PTE_U,
    );

    let stack_frame = mem::alloc_frame().expect("out of frames");
    mem::map_4k(
        pml4 as *mut u64,
        USER_STACK_VA,
        stack_frame,
        mem::PTE_P | mem::PTE_W | mem::PTE_U,
    );

    let kstack = mem::alloc_frame().expect("out of frames");
    let kstack_top = kstack + 0x1000;
    let fp = (kstack_top - core::mem::size_of::<IrqFrame>() as u64) as *mut IrqFrame;
    fp.write(IrqFrame {
        r15: 0, r14: 0, r13: 0, r12: 0, r11: 0, r10: 0, r9: 0, r8: 0,
        rbp: 0, rdi: 0, rsi: 0, rdx: 0, rcx: 0, rbx: 0, rax: 0,
        rip: USER_CODE_VA,
        cs: seg::UCODE as u64,
        rflags: 0x202, // IF=1 — timer interrupts are live in ring 3
        rsp: USER_STACK_VA + 0x1000,
        ss: seg::UDATA as u64,
    });
    procs[i] = Proc { pml4, kstack_top, frame: fp, state: READY, brk: USER_DATA_VA };
}

/// Save `frame` into the current process, pick the next READY process
/// (round-robin), point `TSS.rsp0` at its kernel stack, switch CR3, and
/// return its saved frame for `irq_tail` to resume.
unsafe fn schedule(frame: *mut IrqFrame) -> *mut IrqFrame {
    let procs = &mut *core::ptr::addr_of_mut!(PROCS);
    let cur = *core::ptr::addr_of!(CUR);
    procs[cur].frame = frame;
    for off in 1..=NPROC {
        let next = (cur + off) % NPROC;
        if procs[next].state == READY {
            *core::ptr::addr_of_mut!(CUR) = next;
            seg::set_rsp0(procs[next].kstack_top);
            mem::switch_cr3(procs[next].pml4);
            return procs[next].frame;
        }
    }
    // Nothing else is runnable: resume the current process if it is alive,
    // otherwise every process is dead and the machine can stop.
    if procs[cur].state != READY {
        serial::write_str("all processes dead\n");
        loop {
            core::arch::asm!("cli; hlt");
        }
    }
    frame
}

/// IRQ0: acknowledge the PIC, then treat the tick as an involuntary yield.
/// Before start() arms the scheduler, a tick is acknowledged and dropped —
/// a pre-boot IRQ0 can sit latched in the PIC and fire on the first unmask.
#[no_mangle]
extern "C" fn timer_handler(frame: *mut IrqFrame) -> *mut IrqFrame {
    unsafe {
        pic::eoi();
        if !core::ptr::addr_of!(SCHED_ACTIVE).read() {
            frame
        } else {
            schedule(frame)
        }
    }
}

/// Syscall ABI: rax = number (1 write_char, 2 exit, 3 yield, 4 fork,
/// 5 exec, 6 sbrk), rdi = arg. write/sbrk resume the same frame; the others
/// may return a different process's frame (yield/exit/fork scheduling) or a
/// rewritten one (exec).
#[no_mangle]
unsafe extern "C" fn syscall_handler(frame: *mut IrqFrame) -> *mut IrqFrame {
    let f = &mut *frame;
    match f.rax {
        1 => {
            serial::write_byte(f.rdi as u8);
            frame
        }
        2 => {
            (*core::ptr::addr_of_mut!(PROCS))[*core::ptr::addr_of!(CUR)].state = DEAD;
            schedule(frame)
        }
        3 => schedule(frame),
        4 => {
            // fork: clone the user tables read-only-shared (COW), copy the
            // live frame onto the child's kernel stack, and give the child
            // rax=0 while the parent sees 1. Resumes the same frame — the
            // child is dispatched by a later yield/exit/tick.
            let procs = &mut *core::ptr::addr_of_mut!(PROCS);
            let cur = *core::ptr::addr_of!(CUR);
            match procs.iter().position(|p| p.state == EMPTY) {
                Some(i) => {
                    let cp4 = mem::clone_user_table(procs[cur].pml4 as *mut u64);
                    let kstack = mem::alloc_frame().expect("out of frames for kstack");
                    let ktop = kstack + 0x1000;
                    let cfp = (ktop - core::mem::size_of::<IrqFrame>() as u64) as *mut IrqFrame;
                    core::ptr::copy_nonoverlapping(f, cfp, 1);
                    (*cfp).rax = 0;
                    procs[i] = Proc {
                        pml4: cp4,
                        kstack_top: ktop,
                        frame: cfp,
                        state: READY,
                        brk: procs[cur].brk,
                    };
                    f.rax = 1;
                    // Parent's pages just lost W: flush its TLB so a stale
                    // writable translation can't bypass the COW fault.
                    mem::switch_cr3(procs[cur].pml4);
                }
                None => f.rax = u64::MAX,
            }
            frame
        }
        5 => {
            // exec: replace the user address space with a fresh table
            // running the embedded prog_c image, and rewrite the current
            // frame so iretq enters the new program in ring 3.
            // Old user frames/tables are leaked — a real exec frees them;
            // the upgrade path is a table walker over the private PD chain.
            let code = core::slice::from_raw_parts(
                &prog_c_start as *const u8,
                &prog_c_end as *const u8 as usize - &prog_c_start as *const u8 as usize,
            );
            let pml4 = mem::build_user_table();
            let cf = mem::alloc_frame().expect("out of frames for exec code");
            core::ptr::copy_nonoverlapping(code.as_ptr(), cf as *mut u8, code.len());
            mem::map_4k(pml4 as *mut u64, USER_CODE_VA, cf, mem::PTE_P | mem::PTE_W | mem::PTE_U);
            let sf = mem::alloc_frame().expect("out of frames for exec stack");
            mem::map_4k(pml4 as *mut u64, USER_STACK_VA, sf, mem::PTE_P | mem::PTE_W | mem::PTE_U);
            let procs = &mut *core::ptr::addr_of_mut!(PROCS);
            let cur = *core::ptr::addr_of!(CUR);
            procs[cur].pml4 = pml4;
            procs[cur].brk = USER_DATA_VA;
            *f = IrqFrame {
                r15: 0, r14: 0, r13: 0, r12: 0, r11: 0, r10: 0, r9: 0, r8: 0,
                rbp: 0, rdi: 0, rsi: 0, rdx: 0, rcx: 0, rbx: 0, rax: 0,
                rip: USER_CODE_VA,
                cs: seg::UCODE as u64,
                rflags: 0x202,
                rsp: USER_STACK_VA + 0x1000,
                ss: seg::UDATA as u64,
            };
            mem::switch_cr3(pml4);
            frame
        }
        6 => {
            // sbrk: lazily grow the demand region — no pages are mapped yet;
            // the first touch of each page faults and pf_handler maps it.
            let p = &mut (*core::ptr::addr_of_mut!(PROCS))[*core::ptr::addr_of!(CUR)];
            let inc = (f.rdi + 0xFFF) & !0xFFF;
            f.rax = p.brk;
            if p.brk + inc <= BRK_MAX {
                p.brk += inc;
            }
            frame
        }
        _ => frame,
    }
}

/// Page fault (vector 14): `isr_pf` hands over the iret frame and the CPU
/// error code (err bit0 = page present, bit1 = write access, bit2 = user
/// mode). A not-present fault inside [USER_DATA_VA, brk) is demand paging:
/// allocate a frame, map it U|W, invlpg, and retry the faulting instruction.
/// Anything else still prints EX0E and halts.
#[no_mangle]
extern "C" fn pf_handler(frame: *mut IrqFrame, err: u64) -> *mut IrqFrame {
    unsafe {
        let cr2: u64;
        core::arch::asm!("mov {}, cr2", out(reg) cr2);
        let procs = &mut *core::ptr::addr_of_mut!(PROCS);
        let cur = *core::ptr::addr_of!(CUR);
        if err & 1 == 0 && cr2 >= USER_DATA_VA && cr2 < procs[cur].brk {
            let page = cr2 & !0xFFF;
            let fr = mem::alloc_frame().expect("out of frames for demand page");
            core::ptr::write_bytes(fr as *mut u8, 0, 4096);
            mem::map_4k(
                procs[cur].pml4 as *mut u64,
                page,
                fr,
                mem::PTE_P | mem::PTE_W | mem::PTE_U,
            );
            core::arch::asm!("invlpg [{}]", in(reg) page, options(nostack));
            return frame;
        }
        // Copy-on-write: present + write fault on a private user page whose
        // PTE lost W at fork. Give the faulting process a private copy and
        // retry; the other side keeps the shared frame read-only.
        if err & 3 == 3 && cr2 >= USER_CODE_VA && cr2 < USER_REGION_END {
            let pte = mem::find_pte(procs[cur].pml4 as *const u64, cr2);
            if !pte.is_null()
                && *pte & (mem::PTE_P | mem::PTE_U | mem::PTE_W) == (mem::PTE_P | mem::PTE_U)
            {
                let fr = mem::alloc_frame().expect("out of frames for COW");
                core::ptr::copy_nonoverlapping(
                    (*pte & !0xFFF) as *const u8,
                    fr as *mut u8,
                    4096,
                );
                *pte = fr | (*pte & 0xFFF) | mem::PTE_W;
                core::arch::asm!("invlpg [{}]", in(reg) cr2 & !0xFFF, options(nostack));
                return frame;
            }
        }
        let f = &*frame;
        serial::write_str("EX0E cr2=");
        serial::write_hex(cr2);
        serial::write_str(" err=");
        serial::write_hex(err);
        serial::write_str(" rip=");
        serial::write_hex(f.rip);
        serial::write_str(" rsi=");
        serial::write_hex(f.rsi);
        serial::write_str(" rax=");
        serial::write_hex(f.rax);
        serial::write_str("\n");
        loop {
            core::arch::asm!("cli; hlt");
        }
    }
}

/// Start the scheduler: enter the first READY process and never come back.
pub unsafe fn start() -> ! {
    let procs = &mut *core::ptr::addr_of_mut!(PROCS);
    let first = procs.iter().position(|p| p.state == READY).expect("no procs");
    *core::ptr::addr_of_mut!(CUR) = first;
    seg::set_rsp0(procs[first].kstack_top);
    mem::switch_cr3(procs[first].pml4);
    // Drain any IRQ0 latched during boot (BIOS leaves the PIT ticking at
    // ~18.2 Hz): unmask long enough for it to deliver, the inactive-gate
    // tick handler EOI's and drops it, then re-mask. Without this it would
    // preempt the very first user instruction.
    core::arch::asm!("sti; nop; nop; nop; cli", options(nomem, nostack));
    *core::ptr::addr_of_mut!(SCHED_ACTIVE) = true;
    enter_user(procs[first].frame)
}
