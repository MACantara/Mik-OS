//! Mik-64 kernel source for the MVP.
//!
//! Because Mik-64 is a custom ISA, the kernel is hand-assembled here using
//! `mik_emu::encode`. This crate produces the flat binary that the emulator
//! loads. The real Rust Mik OS for x86-64 will come later.

use mik_emu::{encode, CSR_EPC, CSR_PMODE, CSR_PTBR, CSR_TIMER};

const PTE_P: u64 = 1 << 0;
const PTE_W: u64 = 1 << 1;
const PTE_U: u64 = 1 << 2;

// Process-table layout: two 160-byte slots inside the reserved metadata page
// 0x700000 (the bump allocator only hands out 0x701000+, so it can never
// collide). Each slot stores x1..x14 (x_i at (i-1)*8), then pc, ptbr, pt4,
// state. x15 is kernel-reserved: user programs must not rely on it, and
// handlers use it as the slot pointer / SRET target. x1, x3, x10, x11 are also
// clobbered by any trap or fault.
const PROC_TABLE: i64 = 0x700100;
const SLOT_STRIDE: i64 = 160;
const SLOT_PC: i64 = 112;
const SLOT_PTBR: i64 = 120;
const SLOT_PT4: i64 = 128;
const SLOT_STATE: i64 = 136;
const CUR_SLOT: i64 = 0x700040;
const KERN_PD: i64 = 0x700018;
const CHILD_SLOT: i64 = 0x700068;
const PTE_SCRATCH: i64 = 0x700058;
const TBL_PML4: i64 = 0x700070;
const TBL_PDPT: i64 = 0x700078;
const TBL_PD: i64 = 0x700080;
const TBL_PT4: i64 = 0x700088;
const TBL_UPAGE: i64 = 0x700090;
const USER_VA: i64 = 0x800000;

const INIT_S: &str = include_str!("../user/init.s");
const PROG1_S: &str = include_str!("../user/prog1.s");

/// Label patch slots inside `build_common` that callers may need to fix up.
struct Fixups {
    string_idx: usize,
    exec_addr_idx: usize,
    exec_len_idx: usize,
}

/// Tiny one-pass assembler with label fixups.
struct Asm<'a> {
    code: Vec<u64>,
    labels: std::collections::HashMap<&'a str, usize>,
    unresolved: Vec<(usize, &'a str, Unresolved)>,
}

enum Unresolved {
    Jmp,
    Beq,
    Bne,
    Li,
}

impl<'a> Asm<'a> {
    fn new() -> Self {
        Self {
            code: Vec::new(),
            labels: std::collections::HashMap::new(),
            unresolved: Vec::new(),
        }
    }

    fn label(&mut self, name: &'a str) {
        self.labels.insert(name, self.code.len());
    }

    fn emit(&mut self, word: u64) {
        self.code.push(word);
    }

    fn jmp(&mut self, target: &'a str) {
        let idx = self.code.len();
        self.emit(encode(0x0D, 0, 0, 0, 0));
        self.unresolved.push((idx, target, Unresolved::Jmp));
    }

    fn beq(&mut self, rs1: usize, rs2: usize, target: &'a str) {
        let idx = self.code.len();
        self.emit(encode(0x0B, 0, rs1 as u8, rs2 as u8, 0));
        self.unresolved.push((idx, target, Unresolved::Beq));
    }

    fn bne(&mut self, rs1: usize, rs2: usize, target: &'a str) {
        let idx = self.code.len();
        self.emit(encode(0x0C, 0, rs1 as u8, rs2 as u8, 0));
        self.unresolved.push((idx, target, Unresolved::Bne));
    }

    fn li(&mut self, rd: usize, target: &'a str) {
        let idx = self.code.len();
        self.emit(encode(0x01, rd as u8, 0, 0, 0));
        self.unresolved.push((idx, target, Unresolved::Li));
    }

    fn resolve(&mut self, load_addr: u64) {
        for (idx, target, kind) in &self.unresolved {
            let t = *self.labels.get(*target).expect("undefined label");
            let old = self.code[*idx];
            let imm: i64 = match kind {
                Unresolved::Jmp | Unresolved::Beq | Unresolved::Bne => {
                    // Target PC = base + imm * 8. base = load + idx*8.
                    // target_pc = load + t*8.
                    // imm = (target_pc - base) / 8 = t - idx.
                    (t as i64) - (*idx as i64)
                }
                Unresolved::Li => (load_addr as i64) + (t as i64) * 8, // absolute address
            };
            let mask = (1u64 << 44) - 1;
            let mut new = old & !mask;
            new |= (imm as u64) & mask;
            self.code[*idx] = new;
        }
    }

    fn binary(self, string: &[u8]) -> Vec<u8> {
        let mut program: Vec<u8> = Vec::new();
        for word in self.code {
            program.extend_from_slice(&word.to_le_bytes());
        }
        program.extend_from_slice(string);
        program
    }

    fn len(&self) -> usize {
        self.code.len()
    }

    fn patch_imm(&mut self, idx: usize, imm: i64) {
        let old = self.code[idx];
        let mask = (1u64 << 44) - 1;
        let mut new = old & !mask;
        new |= (imm as u64) & mask;
        self.code[idx] = new;
    }
}

/// Common kernel bootstrap: flat setup, identity page tables, and enable paging.
/// Returns patch slots the caller must fill and then append data for.
fn build_common<'a>(a: &mut Asm<'a>) -> Fixups {
    let page_size: i64 = 0x1000;
    let pte_size: i64 = 8;
    let ptes: i64 = 512;
    let flags = (PTE_P | PTE_W) as i64;

    // Register plan for the kernel:
    // x0  : hard-wired zero
    // x1  : scratch / temporary value
    // x2  : demo page / syscall arg / alloc_page return
    // x3  : scratch
    // x4  : PML4
    // x5  : PDPT
    // x6  : PD
    // x7  : PT0
    // x8  : PT1
    // x9  : PT2
    // x10 : PT3
    // x11 : fill_pt start_pa
    // x12 : fill_pt pt_addr
    // x13 : fill_pt flags
    // x14 : demo page (saved)
    // x15 : fill_pt / alloc_page / free_page return address

    a.label("start");

    // next_page = 0x701000 (demo page), then page tables from 0x702000
    a.emit(encode(0x01, 1, 0, 0, 0x701000));
    a.emit(encode(0x0A, 0, 0, 1, 0x700000));

    // trap vector = syscall_handler
    a.li(1, "syscall_handler");
    a.emit(encode(0x0A, 0, 0, 1, 0x2000));

    // page-fault vector = pf_handler
    a.li(1, "pf_handler");
    a.emit(encode(0x0A, 0, 0, 1, 0x2010));

    // allocate demo page
    a.li(15, "after_alloc");
    a.jmp("alloc_page");

    a.label("after_alloc");
    a.emit(encode(0x01, 13, 0, 0, b'!' as i64));
    a.emit(encode(0x09, 0, 2, 13, 0));          // store8 [x2], '!'
    a.emit(encode(0x07, 13, 2, 0, 0));          // load8 x13, [x2]
    a.emit(encode(0x09, 0, 0, 13, 0x1000));     // store8 [0x1000], '!'

    // save demo page, then bump next_page to 0x702000 for page tables
    a.emit(encode(0x02, 14, 2, 0, 0));          // x14 = demo page
    a.emit(encode(0x01, 1, 0, 0, 0x702000));
    a.emit(encode(0x0A, 0, 0, 1, 0x700000));

    // allocate 7 consecutive pages for the page tables
    a.li(15, "after_pgtbl");
    a.jmp("alloc_page");

    a.label("after_pgtbl");
    // x2 = first page (PML4); advance next_page by 6 more pages
    a.emit(encode(0x08, 3, 0, 0, 0x700000));    // load64 x3, [0x700000]
    a.emit(encode(0x03, 3, 3, 0, 6 * page_size));
    a.emit(encode(0x0A, 0, 0, 3, 0x700000));    // store64 [0x700000], x3

    // x4..x10 = PML4, PDPT, PD, PT0..PT3
    a.emit(encode(0x02, 4, 2, 0, 0));
    a.emit(encode(0x03, 5, 4, 0, 0x1000));
    a.emit(encode(0x03, 6, 4, 0, 0x2000));
    a.emit(encode(0x03, 7, 4, 0, 0x3000));
    a.emit(encode(0x03, 8, 4, 0, 0x4000));
    a.emit(encode(0x03, 9, 4, 0, 0x5000));
    a.emit(encode(0x03, 10, 4, 0, 0x6000));

    // Fill PT0 (0-2 MiB), PT1, PT2, PT3.
    a.emit(encode(0x01, 11, 0, 0, 0));
    a.emit(encode(0x02, 12, 7, 0, 0));
    a.li(15, "after_fill0");
    a.jmp("fill_pt");

    a.label("after_fill0");
    a.emit(encode(0x01, 11, 0, 0, 0x200000));
    a.emit(encode(0x02, 12, 8, 0, 0));
    a.li(15, "after_fill1");
    a.jmp("fill_pt");

    a.label("after_fill1");
    a.emit(encode(0x01, 11, 0, 0, 0x400000));
    a.emit(encode(0x02, 12, 9, 0, 0));
    a.li(15, "after_fill2");
    a.jmp("fill_pt");

    a.label("after_fill2");
    a.emit(encode(0x01, 11, 0, 0, 0x600000));
    a.emit(encode(0x02, 12, 10, 0, 0));
    a.li(15, "after_fill3");
    a.jmp("fill_pt");

    a.label("after_fill3");

    // PML4[0] = PDPT | flags
    a.emit(encode(0x02, 1, 5, 0, 0));
    a.emit(encode(0x03, 1, 1, 0, flags));
    a.emit(encode(0x0A, 0, 4, 1, 0));

    // PDPT[0] = PD | flags
    a.emit(encode(0x02, 1, 6, 0, 0));
    a.emit(encode(0x03, 1, 1, 0, flags));
    a.emit(encode(0x0A, 0, 5, 1, 0));

    // PD[0..3] = PT0..PT3 | flags
    a.emit(encode(0x02, 1, 7, 0, 0));
    a.emit(encode(0x03, 1, 1, 0, flags));
    a.emit(encode(0x0A, 0, 6, 1, 0));

    a.emit(encode(0x02, 1, 8, 0, 0));
    a.emit(encode(0x03, 1, 1, 0, flags));
    a.emit(encode(0x0A, 0, 6, 1, 0x8));

    a.emit(encode(0x02, 1, 9, 0, 0));
    a.emit(encode(0x03, 1, 1, 0, flags));
    a.emit(encode(0x0A, 0, 6, 1, 0x10));

    a.emit(encode(0x02, 1, 10, 0, 0));
    a.emit(encode(0x03, 1, 1, 0, flags));
    a.emit(encode(0x0A, 0, 6, 1, 0x18));

    // PTBR = PML4; PMODE = 1
    a.emit(encode(0x02, 1, 4, 0, 0));           // x1 = PML4
    a.emit(encode(0x12, 0, 1, 0, 0));           // wrcsr PTBR, x1
    a.emit(encode(0x01, 1, 0, 0, 1));
    a.emit(encode(0x12, 0, 1, 0, 1));           // wrcsr PMODE, x1
    a.jmp("after_paging");                      // skip subroutines, caller defines this

    // --- Subroutines ---

    a.label("alloc_page");
    a.emit(encode(0x08, 2, 0, 0, 0x700008));    // load64 x2, [0x700008] (free_list_head)
    a.beq(2, 0, "alloc_bump");                  // if head == 0, bump next_page
    a.emit(encode(0x08, 3, 2, 0, 0));           // load64 x3, [x2] (next)
    a.emit(encode(0x0A, 0, 0, 3, 0x700008));    // store64 [0x700008], x3
    a.emit(encode(0x0F, 0, 15, 0, 0));          // jmpr x15 (x2 = page)

    a.label("alloc_bump");
    a.emit(encode(0x08, 2, 0, 0, 0x700000));    // load64 x2, [0x700000]
    a.emit(encode(0x03, 3, 2, 0, page_size));
    a.emit(encode(0x0A, 0, 0, 3, 0x700000));    // store64 [0x700000], x3
    a.emit(encode(0x0F, 0, 15, 0, 0));          // jmpr x15 (x2 = page)

    a.label("free_page");
    a.emit(encode(0x08, 3, 0, 0, 0x700008));    // load64 x3, [0x700008] (old head)
    a.emit(encode(0x0A, 0, 2, 3, 0));           // store64 [x2], x3 (page->next = old)
    a.emit(encode(0x0A, 0, 0, 2, 0x700008));    // store64 [0x700008], x2 (head = page)
    a.emit(encode(0x0F, 0, 15, 0, 0));          // jmpr x15

    a.label("fill_pt");
    a.emit(encode(0x01, 3, 0, 0, ptes));        // x3 = 512
    a.emit(encode(0x01, 13, 0, 0, flags));      // x13 = PTE_P | PTE_W
    a.label("fill_loop");
    a.emit(encode(0x06, 1, 11, 13, 0));         // or x1, x11, x13
    a.emit(encode(0x0A, 0, 12, 1, 0));          // store64 [x12], x1
    a.emit(encode(0x03, 11, 11, 0, page_size));
    a.emit(encode(0x03, 12, 12, 0, pte_size));
    a.emit(encode(0x03, 3, 3, 0, -1));
    a.bne(3, 0, "fill_loop");
    a.emit(encode(0x0F, 0, 15, 0, 0));          // jmpr x15

    a.label("print_string");
    let string_idx = a.len();
    a.emit(encode(0x01, 1, 0, 0, 0));           // x1 = string_addr (patched)

    a.label("loop");
    a.emit(encode(0x07, 2, 1, 0, 0));           // load8 x2, [x1]
    a.beq(2, 0, "done");                        // if x2 == 0, done
    a.emit(encode(0x09, 0, 0, 2, 0x1000));      // store8 [0x1000], x2
    a.emit(encode(0x03, 1, 1, 0, 1));           // x1 += 1
    a.jmp("loop");

    // Syscall numbers: 0 halt, 1 print_char, 2 fork, 3 exec, 4 yield, 5 exit.
    a.label("syscall_handler");
    a.beq(10, 0, "sys_halt");                   // if x10 == 0, halt
    a.emit(encode(0x01, 3, 0, 0, 1));           // x3 = 1
    a.beq(10, 3, "sys_print_char");             // if x10 == 1, print
    a.emit(encode(0x01, 3, 0, 0, 2));
    a.beq(10, 3, "sys_fork");                   // 2 = fork
    a.emit(encode(0x01, 3, 0, 0, 3));
    a.beq(10, 3, "sys_exec");                   // 3 = exec
    a.emit(encode(0x01, 3, 0, 0, 4));
    a.beq(10, 3, "sched_save");                 // 4 = yield -> reschedule
    a.emit(encode(0x01, 3, 0, 0, 5));
    a.beq(10, 3, "sys_exit");                   // 5 = exit
    a.emit(encode(0x10, 0, 0, 0, 0));           // eret

    a.label("sys_print_char");
    a.emit(encode(0x09, 0, 0, 2, 0x1000));      // store8 [0x1000], x2
    a.emit(encode(0x10, 0, 0, 0, 0));           // eret

    a.label("sys_halt");
    a.emit(encode(0x00, 0, 0, 0, 0));           // halt

    a.label("pf_handler");
    // x10 = fault code, x11 = faulting VA. Demand-page only not-present
    // faults in the user region 0x800000..0xA00000 (the per-process PT4);
    // everything else prints F<code> and halts.
    a.emit(encode(0x01, 1, 0, 0, 1));
    a.bne(10, 1, "pf_print");                   // code != not-present -> print
    a.emit(encode(0x01, 3, 0, 0, 0xFF_FFE0_0000)); // PD-index mask
    a.emit(encode(0x05, 1, 11, 3, 0));          // x1 = va & mask
    a.emit(encode(0x01, 3, 0, 0, USER_VA));
    a.bne(1, 3, "pf_print");                    // outside region -> print
    // pte_addr = cur.pt4 + ((va & 0x1FF000) >> 9). The ISA has no shift, so
    // walk: while x1 != 0 { x1 -= 0x1000; x3 += 8 }.
    // ponytail: O(page-index) walk, <=512 iterations per fault; a SRLI
    // opcode would make it one instruction.
    a.emit(encode(0x01, 3, 0, 0, 0x1FF000));
    a.emit(encode(0x05, 1, 11, 3, 0));          // x1 = va & 0x1FF000
    a.emit(encode(0x08, 15, 0, 0, CUR_SLOT));   // x15 = cur slot
    a.emit(encode(0x08, 15, 15, 0, SLOT_PT4));  // x15 = proc's PT4
    a.emit(encode(0x02, 3, 15, 0, 0));          // x3 = pte_addr (mov)
    a.label("pf_idxloop");
    a.beq(1, 0, "pf_gotidx");
    a.emit(encode(0x03, 1, 1, 0, -0x1000));
    a.emit(encode(0x03, 3, 3, 0, 8));
    a.jmp("pf_idxloop");
    a.label("pf_gotidx");
    a.emit(encode(0x0A, 0, 0, 3, PTE_SCRATCH)); // stash pte_addr
    a.li(15, "pf_alloc_ret");
    a.jmp("alloc_page");
    a.label("pf_alloc_ret");                    // x2 = new page PA
    a.emit(encode(0x08, 3, 0, 0, PTE_SCRATCH));
    a.emit(encode(0x01, 1, 0, 0, (PTE_P | PTE_W | PTE_U) as i64));
    a.emit(encode(0x06, 2, 2, 1, 0));           // x2 = pa | P|W|U
    a.emit(encode(0x0A, 0, 3, 2, 0));           // store64 [x3], x2
    a.emit(encode(0x13, 0, 0, 0, 0));           // sfence
    a.emit(encode(0x10, 0, 0, 0, 0));           // eret -> retry faulting instr

    a.label("pf_print");
    // Print "F" followed by the fault code digit, then halt.
    a.emit(encode(0x01, 1, 0, 0, b'F' as i64)); // 'F'
    a.emit(encode(0x09, 0, 0, 1, 0x1000));      // store8 [0x1000], x1
    a.emit(encode(0x03, 1, 10, 0, b'0' as i64)); // x1 = '0' + x10
    a.emit(encode(0x09, 0, 0, 1, 0x1000));      // store8 [0x1000], x1
    a.emit(encode(0x00, 0, 0, 0, 0));           // halt

    // --- Scheduler: save current context, pick the next live slot, restore ---
    a.label("sched_save");                      // interrupt-vector entry point
    a.emit(encode(0x08, 15, 0, 0, CUR_SLOT));   // x15 = cur slot
    for i in 1..=14i64 {
        a.emit(encode(0x0A, 0, 15, i as u8, (i - 1) * 8)); // save x1..x14
    }
    a.emit(encode(0x11, 1, 0, 0, CSR_EPC as i64)); // x1 = interrupted pc
    a.emit(encode(0x0A, 0, 15, 1, SLOT_PC));

    a.label("sched_pick");
    // x15 = cur slot; candidate x3 = the other slot.
    a.emit(encode(0x01, 1, 0, 0, PROC_TABLE));
    a.emit(encode(0x01, 3, 0, 0, PROC_TABLE + SLOT_STRIDE));
    a.beq(15, 1, "sched_pick_have");
    a.emit(encode(0x02, 3, 1, 0, 0));           // cur was slot1 -> cand slot0
    a.label("sched_pick_have");
    a.emit(encode(0x08, 1, 3, 0, SLOT_STATE));  // cand.state
    a.bne(1, 0, "sched_switch");
    a.emit(encode(0x08, 1, 15, 0, SLOT_STATE)); // cur.state
    a.beq(1, 0, "sys_halt");                    // both dead -> halt
    a.jmp("sched_restore");                     // stay in cur
    a.label("sched_switch");
    a.emit(encode(0x0A, 0, 0, 3, CUR_SLOT));    // cur = candidate
    a.emit(encode(0x02, 15, 3, 0, 0));          // x15 = new slot

    a.label("sched_restore");
    a.emit(encode(0x08, 1, 15, 0, SLOT_PTBR));
    a.emit(encode(0x12, 0, 1, 0, CSR_PTBR as i64)); // switch address space
    a.emit(encode(0x01, 1, 0, 0, 1));
    a.emit(encode(0x12, 0, 1, 0, CSR_PMODE as i64)); // keep paging on
    for i in 1..=14i64 {
        a.emit(encode(0x08, i as u8, 15, 0, (i - 1) * 8)); // restore x1..x14
    }
    a.emit(encode(0x08, 15, 15, 0, SLOT_PC));   // x15 = resume pc
    a.emit(encode(0x14, 0, 15, 0, 0));          // sret x15 -> user mode

    a.label("sys_exit");
    a.emit(encode(0x08, 15, 0, 0, CUR_SLOT));
    a.emit(encode(0x0A, 0, 15, 0, SLOT_STATE)); // state = 0
    a.jmp("sched_pick");

    // sys_exec: x2 = program id. Only id 1 exists; the caller's own user page
    // (VA 0x800000) is overwritten and execution restarts at 0x800000.
    // ponytail: single exec target and stale registers on entry; upgrade to a
    // program table and zeroed registers.
    a.label("sys_exec");
    a.emit(encode(0x01, 3, 0, 0, 1));
    a.bne(2, 3, "sys_ret");                     // unknown id -> return
    a.emit(encode(0x01, 3, 0, 0, 0));           // x3 = prog addr (patched)
    let exec_addr_idx = a.len() - 1;
    a.emit(encode(0x01, 1, 0, 0, 0));           // x1 = prog len (patched)
    let exec_len_idx = a.len() - 1;
    a.emit(encode(0x01, 15, 0, 0, USER_VA));    // x15 = dst VA
    a.label("exec_copy");
    a.emit(encode(0x07, 2, 3, 0, 0));           // load8 x2, [x3]
    a.emit(encode(0x09, 0, 15, 2, 0));          // store8 [x15], x2
    a.emit(encode(0x03, 3, 3, 0, 1));
    a.emit(encode(0x03, 15, 15, 0, 1));
    a.emit(encode(0x03, 1, 1, 0, -1));
    a.bne(1, 0, "exec_copy");
    a.emit(encode(0x01, 15, 0, 0, USER_VA));
    a.emit(encode(0x14, 0, 15, 0, 0));          // sret -> run new image
    a.label("sys_ret");
    a.emit(encode(0x10, 0, 0, 0, 0));           // eret

    // sys_fork: clone the current process into the other slot. Builds a full
    // PML4/PDPT/PD/PT4 chain whose PD[0..3] shares the kernel identity map and
    // eagerly copies the caller's user page.
    // ponytail: eager page copy and only the code page is cloned (parent must
    // not have demand-paged extra pages yet); upgrade to COW + PT4 clone walk.
    a.label("sys_fork");
    a.emit(encode(0x08, 15, 0, 0, CUR_SLOT));   // x15 = cur slot
    a.emit(encode(0x01, 1, 0, 0, PROC_TABLE));
    a.emit(encode(0x01, 3, 0, 0, PROC_TABLE + SLOT_STRIDE));
    a.beq(15, 1, "fork_have_slot");
    a.emit(encode(0x02, 3, 1, 0, 0));
    a.label("fork_have_slot");                  // x3 = child slot
    a.emit(encode(0x0A, 0, 0, 3, CHILD_SLOT));
    a.emit(encode(0x02, 15, 3, 0, 0));          // x15 = child slot
    for i in 1..=14i64 {
        a.emit(encode(0x0A, 0, 15, i as u8, (i - 1) * 8)); // child.x_i = live
    }
    a.emit(encode(0x0A, 0, 15, 0, 8));          // child.x2 = 0
    a.emit(encode(0x11, 1, 0, 0, CSR_EPC as i64));
    a.emit(encode(0x0A, 0, 15, 1, SLOT_PC));    // child.pc = epc
    a.emit(encode(0x01, 1, 0, 0, 1));
    a.emit(encode(0x0A, 0, 15, 1, SLOT_STATE)); // child.state = 1
    // allocate pml4, pdpt, pd, pt4, user page into the build_tables scratch
    a.li(15, "fork_a0");
    a.jmp("alloc_page");
    a.label("fork_a0");
    a.emit(encode(0x0A, 0, 0, 2, TBL_PML4));
    a.li(15, "fork_a1");
    a.jmp("alloc_page");
    a.label("fork_a1");
    a.emit(encode(0x0A, 0, 0, 2, TBL_PDPT));
    a.li(15, "fork_a2");
    a.jmp("alloc_page");
    a.label("fork_a2");
    a.emit(encode(0x0A, 0, 0, 2, TBL_PD));
    a.li(15, "fork_a3");
    a.jmp("alloc_page");
    a.label("fork_a3");
    a.emit(encode(0x0A, 0, 0, 2, TBL_PT4));
    a.li(15, "fork_a4");
    a.jmp("alloc_page");
    a.label("fork_a4");
    a.emit(encode(0x0A, 0, 0, 2, TBL_UPAGE));
    a.li(15, "fork_bt_ret");
    a.jmp("build_tables");
    a.label("fork_bt_ret");
    a.emit(encode(0x08, 15, 0, 0, CHILD_SLOT));
    a.emit(encode(0x08, 1, 0, 0, TBL_PML4));
    a.emit(encode(0x0A, 0, 15, 1, SLOT_PTBR));  // child.ptbr = pml4
    a.emit(encode(0x08, 1, 0, 0, TBL_PT4));
    a.emit(encode(0x0A, 0, 15, 1, SLOT_PT4));   // child.pt4 = pt4
    // copy parent's user page (VA 0x800000 under current ptbr) to child page
    a.emit(encode(0x01, 1, 0, 0, USER_VA));     // src
    a.emit(encode(0x08, 3, 0, 0, TBL_UPAGE));   // dst PA
    a.emit(encode(0x01, 15, 0, 0, 512));        // 512 quads
    a.label("fork_copy");
    a.emit(encode(0x08, 2, 1, 0, 0));
    a.emit(encode(0x0A, 0, 3, 2, 0));
    a.emit(encode(0x03, 1, 1, 0, 8));
    a.emit(encode(0x03, 3, 3, 0, 8));
    a.emit(encode(0x03, 15, 15, 0, -1));
    a.bne(15, 0, "fork_copy");
    // parent return value: pid 1 if the child is slot1, else 0
    a.emit(encode(0x01, 2, 0, 0, 0));
    a.emit(encode(0x08, 1, 0, 0, CHILD_SLOT));
    a.emit(encode(0x01, 3, 0, 0, PROC_TABLE + SLOT_STRIDE));
    a.bne(1, 3, "fork_ret");
    a.emit(encode(0x01, 2, 0, 0, 1));
    a.label("fork_ret");
    a.emit(encode(0x10, 0, 0, 0, 0));           // eret -> parent resumes

    // build_tables: fill a PML4/PDPT/PD/PT4 chain from the scratch addrs
    // TBL_PML4/TBL_PDPT/TBL_PD/TBL_PT4/TBL_UPAGE. PD[0..3] is copied from the
    // kernel PD (saved at KERN_PD) so the kernel half is shared; PD[4] gets
    // the process PT4 whose entry 0 maps USER_VA. Returns via x15.
    a.label("build_tables");
    a.emit(encode(0x08, 1, 0, 0, TBL_PML4));
    a.emit(encode(0x08, 2, 0, 0, TBL_PDPT));
    a.emit(encode(0x01, 3, 0, 0, (PTE_P | PTE_W) as i64));
    a.emit(encode(0x06, 2, 2, 3, 0));
    a.emit(encode(0x0A, 0, 1, 2, 0));           // pml4[0] = pdpt|P|W
    a.emit(encode(0x08, 1, 0, 0, TBL_PDPT));
    a.emit(encode(0x08, 2, 0, 0, TBL_PD));
    a.emit(encode(0x06, 2, 2, 3, 0));
    a.emit(encode(0x0A, 0, 1, 2, 0));           // pdpt[0] = pd|P|W
    a.emit(encode(0x08, 1, 0, 0, KERN_PD));     // x1 = kernel PD
    a.emit(encode(0x08, 2, 0, 0, TBL_PD));      // x2 = proc PD
    for i in 0..4i64 {
        a.emit(encode(0x08, 3, 1, 0, i * 8));   // x3 = kPD[i]
        a.emit(encode(0x0A, 0, 2, 3, i * 8));   // pPD[i] = kPD[i]
    }
    a.emit(encode(0x08, 2, 0, 0, TBL_PT4));     // x2 = pt4
    a.emit(encode(0x01, 3, 0, 0, (PTE_P | PTE_W) as i64));
    a.emit(encode(0x06, 3, 2, 3, 0));           // x3 = pt4|P|W
    a.emit(encode(0x08, 1, 0, 0, TBL_PD));      // x1 = proc PD
    a.emit(encode(0x0A, 0, 1, 3, 4 * 8));       // pd[4] = pt4|P|W
    a.emit(encode(0x08, 1, 0, 0, TBL_UPAGE));
    a.emit(encode(0x01, 3, 0, 0, (PTE_P | PTE_W | PTE_U) as i64));
    a.emit(encode(0x06, 3, 1, 3, 0));           // x3 = upage|P|W|U
    a.emit(encode(0x08, 2, 0, 0, TBL_PT4));
    a.emit(encode(0x0A, 0, 2, 3, 0));           // pt4[0] = upage|P|W|U
    a.emit(encode(0x0F, 0, 15, 0, 0));          // jmpr x15

    Fixups {
        string_idx,
        exec_addr_idx,
        exec_len_idx,
    }
}

/// Finalize the assembler, patch the `print_string` string address, and emit the
/// raw binary with the provided trailing string.
fn finalize(mut a: Asm, load_addr: u64, string_idx: usize, string: &[u8]) -> Vec<u8> {
    a.resolve(load_addr);
    let string_addr = load_addr + (a.len() as u64) * 8;
    a.patch_imm(string_idx, string_addr as i64);
    a.binary(string)
}

/// Return the flat Mik-64 kernel binary for the MVP.
pub fn kernel_binary() -> Vec<u8> {
    let load_addr = 0x400000_u64;
    let mut a = Asm::new();
    let fix = build_common(&mut a);

    a.label("after_paging");
    // free the demo page (no-op) and continue
    a.emit(encode(0x02, 2, 14, 0, 0));          // x2 = demo page
    a.li(15, "after_free");
    a.jmp("free_page");

    a.label("after_free");
    a.emit(encode(0x01, 2, 0, 0, b'?' as i64));
    a.emit(encode(0x0E, 0, 0, 0, 1));           // trap 1 -> print '?'

    // print "Mik OS\n"
    a.jmp("print_string");

    a.label("done");
    a.emit(encode(0x0E, 0, 0, 0, 0));           // trap 0 -> halt

    finalize(a, load_addr, fix.string_idx, b"Mik OS\n\0")
}

/// Return a kernel binary that enables paging and then deliberately touches an
/// unmapped virtual address to exercise the kernel page-fault handler.
pub fn kernel_pagefault() -> Vec<u8> {
    let load_addr = 0x400000_u64;
    let mut a = Asm::new();
    let fix = build_common(&mut a);

    a.label("after_paging");
    // Touch an unmapped page (1 GiB) to trigger a not-present page fault.
    a.emit(encode(0x08, 1, 0, 0, 0x400_0000));  // load64 x1, [0x4000000]

    a.label("done");
    a.emit(encode(0x0E, 0, 0, 0, 0));           // trap 0 -> halt

    finalize(a, load_addr, fix.string_idx, b"\0")
}

/// Return a kernel binary that frees the demo page, allocates it again, and
/// writes a sentinel to the reused page. Used to test the physical free list.
pub fn kernel_freelist() -> Vec<u8> {
    let load_addr = 0x400000_u64;
    let mut a = Asm::new();
    let fix = build_common(&mut a);

    a.label("after_paging");
    // Free the demo page, then allocate again and write a sentinel.
    a.emit(encode(0x02, 2, 14, 0, 0));          // x2 = demo page
    a.li(15, "after_free");
    a.jmp("free_page");

    a.label("after_free");
    a.li(15, "after_realloc");
    a.jmp("alloc_page");

    a.label("after_realloc");
    // x2 = reused page (should be the demo page 0x701000).
    a.emit(encode(0x01, 3, 0, 0, 0xCAFEBABE));  // sentinel
    a.emit(encode(0x0A, 0, 2, 3, 0));           // store64 [x2], x3
    a.emit(encode(0x0E, 0, 0, 0, 0));           // trap 0 -> halt

    a.label("done");

    finalize(a, load_addr, fix.string_idx, b"\0")
}

/// Return a kernel binary that maps a user page at 0x800000, copies a tiny
/// user program (TRAP 1; TRAP 0) into it, and SRETs into it. The kernel sets
/// x2 = 'U' so the user TRAP 1 prints the character and returns; the second
/// TRAP halts.
pub fn kernel_user_mode() -> Vec<u8> {
    let load_addr = 0x400000_u64;
    let mut a = Asm::new();
    let fix = build_common(&mut a);

    // User program: TRAP 1 (print x2), then TRAP 0 (halt).
    let user_code: Vec<u8> = [
        encode(0x0E, 0, 0, 0, 1).to_le_bytes().to_vec(),
        encode(0x0E, 0, 0, 0, 0).to_le_bytes().to_vec(),
    ]
    .concat();

    a.label("after_paging");
    // Allocate a new PT for the 8-10 MiB region (PD index 4).
    a.li(15, "after_pt4");
    a.jmp("alloc_page");

    a.label("after_pt4");
    a.emit(encode(0x02, 8, 2, 0, 0));           // x8 = PT4 PA

    // Allocate the user code page.
    a.li(15, "after_user_page");
    a.jmp("alloc_page");

    a.label("after_user_page");
    a.emit(encode(0x02, 7, 2, 0, 0));           // x7 = user code PA

    // Copy the embedded user program into the user code page.
    // Use x9 as a temporary source pointer (PT2 from build_common is no longer needed).
    a.li(9, "user_code_data");
    a.emit(encode(0x08, 1, 9, 0, 0));           // load64 x1, [x9]
    a.emit(encode(0x0A, 0, 7, 1, 0));           // store64 [x7], x1
    a.emit(encode(0x08, 1, 9, 0, 8));           // load64 x1, [x9 + 8]
    a.emit(encode(0x0A, 0, 7, 1, 8));           // store64 [x7 + 8], x1

    // PT4[0] = user code PA | PTE_P | PTE_U
    a.emit(encode(0x02, 1, 7, 0, 0));           // x1 = user code PA
    a.emit(encode(0x03, 1, 1, 0, (PTE_P | PTE_U) as i64));
    a.emit(encode(0x0A, 0, 8, 1, 0));           // store64 [x8], x1

    // PD[4] = PT4 PA | PTE_P | PTE_W (so the walker can descend).
    // The PD is in x6 from build_common.
    a.emit(encode(0x02, 1, 8, 0, 0));           // x1 = PT4 PA
    a.emit(encode(0x03, 1, 1, 0, (PTE_P | PTE_W) as i64));
    a.emit(encode(0x0A, 0, 6, 1, 4 * 8));       // store64 [x6 + 4*8], x1

    // Flush the TLB so the new mapping is visible.
    a.emit(encode(0x13, 0, 0, 0, 0));           // sfence

    // Set the syscall argument and SRET into the user program at 0x800000.
    a.emit(encode(0x01, 2, 0, 0, b'U' as i64)); // x2 = 'U'
    a.emit(encode(0x01, 1, 0, 0, 0x800000));    // x1 = user VA
    a.emit(encode(0x14, 0, 1, 0, 0));           // sret x1

    a.label("done");
    a.emit(encode(0x00, 0, 0, 0, 0));           // halt (fallback)

    a.label("user_code_data");

    finalize(a, load_addr, fix.string_idx, &user_code)
}

/// Return a kernel binary that sets up a programmable interval timer and
/// `SRET`s into a tiny user program that just spins. A timer handler prints
/// 'T' and `IRET`s back; after three ticks the machine halts.
pub fn kernel_timer() -> Vec<u8> {
    let load_addr = 0x400000_u64;
    let mut a = Asm::new();
    let fix = build_common(&mut a);

    // User program: JMP 0 (infinite loop). It is one 64-bit word at the
    // tail of the binary, to be copied into a user page at 0x800000.
    let user_code: Vec<u8> = encode(0x0D, 0, 0, 0, 0).to_le_bytes().to_vec();

    a.label("after_paging");

    // Install the timer interrupt vector at 0x2020.
    a.li(1, "timer_handler");
    a.emit(encode(0x0A, 0, 0, 1, 0x2020));      // store64 [0x2020], x1

    // Initialize a tick counter in the allocator's metadata area.
    a.emit(encode(0x0A, 0, 0, 0, 0x700010));    // store64 [0x700010], x0

    // Set the timer interval (100 steps) and start it.
    a.emit(encode(0x01, 1, 0, 0, 100));
    a.emit(encode(0x12, 0, 1, 0, CSR_TIMER as i64)); // wrcsr TIMER, x1

    // Allocate a new PT for the 8-10 MiB region (PD index 4).
    a.li(15, "after_pt4");
    a.jmp("alloc_page");

    a.label("after_pt4");
    a.emit(encode(0x02, 8, 2, 0, 0));           // x8 = PT4 PA

    // Allocate the user code page.
    a.li(15, "after_user_page");
    a.jmp("alloc_page");

    a.label("after_user_page");
    a.emit(encode(0x02, 7, 2, 0, 0));           // x7 = user code PA

    // Copy the embedded user program into the user code page.
    a.li(9, "user_code_data");
    a.emit(encode(0x08, 1, 9, 0, 0));           // load64 x1, [x9]
    a.emit(encode(0x0A, 0, 7, 1, 0));           // store64 [x7], x1

    // PT4[0] = user code PA | PTE_P | PTE_U
    a.emit(encode(0x02, 1, 7, 0, 0));           // x1 = user code PA
    a.emit(encode(0x03, 1, 1, 0, (PTE_P | PTE_U) as i64));
    a.emit(encode(0x0A, 0, 8, 1, 0));           // store64 [x8], x1

    // PD[4] = PT4 PA | PTE_P | PTE_W
    a.emit(encode(0x02, 1, 8, 0, 0));           // x1 = PT4 PA
    a.emit(encode(0x03, 1, 1, 0, (PTE_P | PTE_W) as i64));
    a.emit(encode(0x0A, 0, 6, 1, 4 * 8));       // store64 [x6 + 4*8], x1

    // Flush the TLB so the new mapping is visible.
    a.emit(encode(0x13, 0, 0, 0, 0));           // sfence

    // SRET into the user program at 0x800000.
    a.emit(encode(0x01, 1, 0, 0, 0x800000));    // x1 = user VA
    a.emit(encode(0x14, 0, 1, 0, 0));           // sret x1

    a.label("done");
    a.emit(encode(0x00, 0, 0, 0, 0));           // halt (used by the timer)

    a.label("timer_handler");
    // Increment the tick counter.
    a.emit(encode(0x08, 1, 0, 0, 0x700010));    // load64 x1, [0x700010]
    a.emit(encode(0x03, 1, 1, 0, 1));           // addi x1, x1, 1
    a.emit(encode(0x0A, 0, 0, 1, 0x700010));    // store64 [0x700010], x1
    // Print 'T'.
    a.emit(encode(0x01, 2, 0, 0, b'T' as i64)); // x2 = 'T'
    a.emit(encode(0x09, 0, 0, 2, 0x1000));      // store8 [0x1000], x2
    // If three ticks, halt; otherwise IRET back to user mode.
    a.emit(encode(0x01, 2, 0, 0, 3));           // x2 = 3
    a.beq(1, 2, "done");
    a.emit(encode(0x16, 0, 0, 0, 0));           // iret

    a.label("user_code_data");

    finalize(a, load_addr, fix.string_idx, &user_code)
}

/// Return a kernel binary that boots a miniature OS: two process slots with
/// per-process page tables, timer-driven round-robin scheduling, `fork` /
/// `exec` / `yield` / `exit` syscalls, and demand paging in the user region.
/// The user programs are assembled by `mik-asm` at kernel-build time and
/// appended after the kernel code.
pub fn kernel_os() -> Vec<u8> {
    let load_addr = 0x400000_u64;
    let init_bin = mik_asm::assemble(INIT_S, USER_VA as u64).expect("init assembles");
    let prog1_bin = mik_asm::assemble(PROG1_S, USER_VA as u64).expect("prog1 assembles");
    assert!(
        init_bin.len() <= 0x1000 && prog1_bin.len() <= 0x1000,
        "user programs must fit in one page"
    );

    let mut a = Asm::new();
    let fix = build_common(&mut a);

    a.label("after_paging");
    // Save the kernel PD for build_tables' shared-PD copy.
    a.emit(encode(0x0A, 0, 0, 6, KERN_PD));     // [0x700018] = x6
    // Install the scheduler at the interrupt vector.
    a.li(1, "sched_save");
    a.emit(encode(0x0A, 0, 0, 1, 0x2020));
    // Allocate pml4, pdpt, pd, pt4, user page for proc0.
    a.li(15, "os_a0");
    a.jmp("alloc_page");
    a.label("os_a0");
    a.emit(encode(0x0A, 0, 0, 2, TBL_PML4));
    a.li(15, "os_a1");
    a.jmp("alloc_page");
    a.label("os_a1");
    a.emit(encode(0x0A, 0, 0, 2, TBL_PDPT));
    a.li(15, "os_a2");
    a.jmp("alloc_page");
    a.label("os_a2");
    a.emit(encode(0x0A, 0, 0, 2, TBL_PD));
    a.li(15, "os_a3");
    a.jmp("alloc_page");
    a.label("os_a3");
    a.emit(encode(0x0A, 0, 0, 2, TBL_PT4));
    a.li(15, "os_a4");
    a.jmp("alloc_page");
    a.label("os_a4");
    a.emit(encode(0x0A, 0, 0, 2, TBL_UPAGE));
    a.li(15, "os_bt_ret");
    a.jmp("build_tables");
    a.label("os_bt_ret");
    // Fill slot 0: ptbr, pt4, pc = USER_VA, state = 1 (regs stay zero).
    a.emit(encode(0x08, 1, 0, 0, TBL_PML4));
    a.emit(encode(0x0A, 0, 0, 1, PROC_TABLE + SLOT_PTBR));
    a.emit(encode(0x08, 1, 0, 0, TBL_PT4));
    a.emit(encode(0x0A, 0, 0, 1, PROC_TABLE + SLOT_PT4));
    a.emit(encode(0x01, 1, 0, 0, USER_VA));
    a.emit(encode(0x0A, 0, 0, 1, PROC_TABLE + SLOT_PC));
    a.emit(encode(0x01, 1, 0, 0, 1));
    a.emit(encode(0x0A, 0, 0, 1, PROC_TABLE + SLOT_STATE));
    // Copy the init binary into proc0's user page (via its physical address).
    a.emit(encode(0x01, 3, 0, 0, 0));           // x3 = init addr (patched)
    let init_addr_idx = a.len() - 1;
    a.emit(encode(0x01, 1, 0, 0, 0));           // x1 = init len (patched)
    let init_len_idx = a.len() - 1;
    a.emit(encode(0x08, 15, 0, 0, TBL_UPAGE));  // x15 = dst page PA
    a.label("os_init_copy");
    a.emit(encode(0x07, 2, 3, 0, 0));           // load8 x2, [x3]
    a.emit(encode(0x09, 0, 15, 2, 0));          // store8 [x15], x2
    a.emit(encode(0x03, 3, 3, 0, 1));
    a.emit(encode(0x03, 15, 15, 0, 1));
    a.emit(encode(0x03, 1, 1, 0, -1));
    a.bne(1, 0, "os_init_copy");
    // cur = slot0, start the timer, and dispatch proc0.
    a.emit(encode(0x01, 1, 0, 0, PROC_TABLE));
    a.emit(encode(0x0A, 0, 0, 1, CUR_SLOT));
    a.emit(encode(0x01, 1, 0, 0, 100));
    a.emit(encode(0x12, 0, 1, 0, CSR_TIMER as i64));
    a.emit(encode(0x08, 15, 0, 0, CUR_SLOT));
    a.jmp("sched_restore");
    a.label("done");
    a.emit(encode(0x00, 0, 0, 0, 0));           // fallback halt

    a.resolve(load_addr);
    let blob_base = load_addr + (a.len() as u64) * 8;
    a.patch_imm(fix.string_idx, blob_base as i64); // unused by kernel_os
    a.patch_imm(init_addr_idx, blob_base as i64);
    a.patch_imm(init_len_idx, init_bin.len() as i64);
    a.patch_imm(fix.exec_addr_idx, (blob_base + init_bin.len() as u64) as i64);
    a.patch_imm(fix.exec_len_idx, prog1_bin.len() as i64);
    let mut blob = init_bin;
    blob.extend_from_slice(&prog1_bin);
    a.binary(&blob)
}
