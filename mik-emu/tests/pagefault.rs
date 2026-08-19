use mik_emu::{encode, Machine, CSR_PTBR, CSR_PMODE};

const PAGE_SIZE: u64 = 4096;
const PTE_P: u64 = 1 << 0;
const PTE_W: u64 = 1 << 1;

fn phys_write64(m: &mut Machine, addr: u64, val: u64) {
    let bytes = val.to_le_bytes();
    m.mem[addr as usize..addr as usize + 8].copy_from_slice(&bytes);
}

fn phys_read64(m: &Machine, addr: u64) -> u64 {
    let bytes = &m.mem[addr as usize..addr as usize + 8];
    u64::from_le_bytes(bytes.try_into().unwrap())
}

/// Add one 4 KiB mapping to existing page tables. Creates missing levels
/// using fresh pages starting from `*next`. Returns the new `*next` value.
fn map_page(m: &mut Machine, root: u64, next: &mut u64, va: u64, pa: u64, flags: u64) {
    let pml4_idx = (va >> 39) & 0x1FF;
    let pdpt_idx = (va >> 30) & 0x1FF;
    let pd_idx = (va >> 21) & 0x1FF;
    let pt_idx = (va >> 12) & 0x1FF;

    // PML4[entry] -> PDPT
    let mut pml4_entry = phys_read64(m, root + pml4_idx * 8);
    if pml4_entry & PTE_P == 0 {
        pml4_entry = (*next & !0xFFF) | PTE_P | PTE_W;
        phys_write64(m, root + pml4_idx * 8, pml4_entry);
        *next += PAGE_SIZE;
    }
    let pdpt = (pml4_entry & !0xFFF) >> 12;

    // PDPT[entry] -> PD
    let mut pdpt_entry = phys_read64(m, (pdpt << 12) + pdpt_idx * 8);
    if pdpt_entry & PTE_P == 0 {
        pdpt_entry = (*next & !0xFFF) | PTE_P | PTE_W;
        phys_write64(m, (pdpt << 12) + pdpt_idx * 8, pdpt_entry);
        *next += PAGE_SIZE;
    }
    let pd = (pdpt_entry & !0xFFF) >> 12;

    // PD[entry] -> PT
    let mut pd_entry = phys_read64(m, (pd << 12) + pd_idx * 8);
    if pd_entry & PTE_P == 0 {
        pd_entry = (*next & !0xFFF) | PTE_P | PTE_W;
        phys_write64(m, (pd << 12) + pd_idx * 8, pd_entry);
        *next += PAGE_SIZE;
    }
    let pt = (pd_entry & !0xFFF) >> 12;

    // PT[entry] -> page
    phys_write64(
        m,
        (pt << 12) + pt_idx * 8,
        (pa & !0xFFF) | PTE_P | flags,
    );
}

#[test]
fn page_fault_is_delivered_to_handler() {
    // Build a program that:
    // 1. Sets up an identity map for its own code page.
    // 2. Installs a page-fault handler at 0x2010.
    // 3. Enables paging.
    // 4. Tries to load from an unmapped VA (0x500000) — should page-fault.
    // 5. The handler prints 'F' and halts.
    let load_addr = 0x400000_u64;
    let code = vec![
        // 0: li x1, pml4_addr (will be set below)
        encode(0x01, 1, 0, 0, 0), // placeholder
        // 1: wrcsr PTBR, x1
        encode(0x12, 0, 1, 0, 0),
        // 2: li x1, 1
        encode(0x01, 1, 0, 0, 1),
        // 3: wrcsr PMODE, x1
        encode(0x12, 0, 1, 0, 1),
        // 4: load8 x2, [x0 + 0x500000]  (unmapped — will page-fault)
        encode(0x07, 2, 0, 0, 0x500000),
        // 5: halt (should not reach here)
        encode(0x00, 0, 0, 0, 0),
        // 6: (page-fault handler starts here) li x3, 'F'
        encode(0x01, 3, 0, 0, b'F' as i64),
        // 7: store8 [0x1000], x3
        encode(0x09, 0, 0, 3, 0x1000),
        // 8: halt
        encode(0x00, 0, 0, 0, 0),
    ];

    let mut program: Vec<u8> = Vec::new();
    for word in code {
        program.extend_from_slice(&word.to_le_bytes());
    }

    let mut m = Machine::new();
    m.load_binary(&program);

    // Build an identity map for the code page (0x400000) and the serial page (0x1000).
    let pml4 = 0x800000_u64;
    let mut next = 0x801000_u64;
    map_page(&mut m, pml4, &mut next, load_addr, load_addr, PTE_W);
    map_page(&mut m, pml4, &mut next, 0x1000, 0x1000, PTE_W);

    // Install the page-fault handler address at 0x2010.
    let handler_addr = load_addr + 6 * 8; // instruction 6
    phys_write64(&mut m, 0x2010, handler_addr);

    // Fix up the pml4 address in instruction 0.
    phys_write64(&mut m, load_addr, encode(0x01, 1, 0, 0, pml4 as i64));

    m.csrs[CSR_PTBR as usize] = pml4;
    m.csrs[CSR_PMODE as usize] = 1;

    let mut output = Vec::new();
    while !m.halted {
        m.step(&mut output).expect("step should succeed");
    }
    assert_eq!(m.exit_code, 0);
    assert_eq!(output, b"F");
}

#[test]
fn identity_mapped_program_runs_with_paging_enabled() {
    // A simple program that prints 'P' and halts, running with paging on.
    let load_addr = 0x400000_u64;
    let code = vec![
        // 0: li x1, pml4_addr (placeholder)
        encode(0x01, 1, 0, 0, 0),
        // 1: wrcsr PTBR, x1
        encode(0x12, 0, 1, 0, 0),
        // 2: li x1, 1
        encode(0x01, 1, 0, 0, 1),
        // 3: wrcsr PMODE, x1
        encode(0x12, 0, 1, 0, 1),
        // 4: li x1, 'P'
        encode(0x01, 1, 0, 0, b'P' as i64),
        // 5: store8 [0x1000], x1
        encode(0x09, 0, 0, 1, 0x1000),
        // 6: halt
        encode(0x00, 0, 0, 0, 0),
    ];

    let mut program: Vec<u8> = Vec::new();
    for word in code {
        program.extend_from_slice(&word.to_le_bytes());
    }

    let mut m = Machine::new();
    m.load_binary(&program);

    let pml4 = 0x800000_u64;
    let mut next = 0x801000_u64;
    map_page(&mut m, pml4, &mut next, load_addr, load_addr, PTE_W);
    map_page(&mut m, pml4, &mut next, 0x1000, 0x1000, PTE_W);

    // Fix up the pml4 address in instruction 0.
    phys_write64(&mut m, load_addr, encode(0x01, 1, 0, 0, pml4 as i64));

    m.csrs[CSR_PTBR as usize] = pml4;
    m.csrs[CSR_PMODE as usize] = 1;

    let mut output = Vec::new();
    while !m.halted {
        m.step(&mut output).expect("step should succeed");
    }
    assert_eq!(output, b"P");
}
