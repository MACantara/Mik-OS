//! Mik-64 kernel source for the MVP.
//!
//! Because Mik-64 is a custom ISA, the kernel is hand-assembled here using
//! `mik_emu::encode`. This crate produces the flat binary that the emulator
//! loads. The real Rust Mik OS for x86-64 will come later.

use mik_emu::encode;

const PTE_P: u64 = 1 << 0;
const PTE_W: u64 = 1 << 1;

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
/// Returns the index of the `print_string` string LI that the caller must patch
/// and then append a string.
fn build_common<'a>(a: &mut Asm<'a>) -> usize {
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
    a.emit(encode(0x08, 2, 0, 0, 0x700000));    // load64 x2, [0x700000]
    a.emit(encode(0x03, 3, 2, 0, page_size));
    a.emit(encode(0x0A, 0, 0, 3, 0x700000));    // store64 [0x700000], x3
    a.emit(encode(0x0F, 0, 15, 0, 0));          // jmpr x15

    a.label("free_page");
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

    a.label("syscall_handler");
    a.beq(10, 0, "sys_halt");                   // if x10 == 0, halt
    a.emit(encode(0x01, 3, 0, 0, 1));           // x3 = 1
    a.beq(10, 3, "sys_print_char");             // if x10 == 1, print
    a.emit(encode(0x10, 0, 0, 0, 0));           // eret

    a.label("sys_print_char");
    a.emit(encode(0x09, 0, 0, 2, 0x1000));      // store8 [0x1000], x2
    a.emit(encode(0x10, 0, 0, 0, 0));           // eret

    a.label("sys_halt");
    a.emit(encode(0x00, 0, 0, 0, 0));           // halt

    a.label("pf_handler");
    // Print "F" followed by the fault code digit, then halt.
    a.emit(encode(0x01, 1, 0, 0, b'F' as i64)); // 'F'
    a.emit(encode(0x09, 0, 0, 1, 0x1000));      // store8 [0x1000], x1
    a.emit(encode(0x03, 1, 10, 0, b'0' as i64)); // x1 = '0' + x10
    a.emit(encode(0x09, 0, 0, 1, 0x1000));      // store8 [0x1000], x1
    a.emit(encode(0x00, 0, 0, 0, 0));           // halt

    string_idx
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
    let string_idx = build_common(&mut a);

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

    finalize(a, load_addr, string_idx, b"Mik OS\n\0")
}

/// Return a kernel binary that enables paging and then deliberately touches an
/// unmapped virtual address to exercise the kernel page-fault handler.
pub fn kernel_pagefault() -> Vec<u8> {
    let load_addr = 0x400000_u64;
    let mut a = Asm::new();
    let string_idx = build_common(&mut a);

    a.label("after_paging");
    // Touch an unmapped page (1 GiB) to trigger a not-present page fault.
    a.emit(encode(0x08, 1, 0, 0, 0x400_0000));  // load64 x1, [0x4000000]

    a.label("done");
    a.emit(encode(0x0E, 0, 0, 0, 0));           // trap 0 -> halt

    finalize(a, load_addr, string_idx, b"\0")
}
