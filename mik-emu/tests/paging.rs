use mik_emu::{Machine, CSR_PTBR, CSR_PMODE};

const PAGE_SIZE: u64 = 4096;
const PTE_P: u64 = 1 << 0;
const PTE_W: u64 = 1 << 1;
const PTE_NX: u64 = 1 << 63;

/// Write a 64-bit value to the machine's physical memory.
fn phys_write64(m: &mut Machine, addr: u64, val: u64) {
    let bytes = val.to_le_bytes();
    m.mem[addr as usize..addr as usize + 8].copy_from_slice(&bytes);
}

fn phys_read64(m: &Machine, addr: u64) -> u64 {
    let bytes = &m.mem[addr as usize..addr as usize + 8];
    u64::from_le_bytes(bytes.try_into().unwrap())
}

/// Build a 4-level identity map for a single 4 KiB page at virtual address
/// `va` mapping to physical address `pa`. Returns the physical address of the
/// PML4 root.
fn build_identity_map(m: &mut Machine, va: u64, pa: u64, flags: u64) -> u64 {
    // Allocate page tables at a known physical region (0x800000 onward).
    let mut next_page = 0x800000_u64;

    let pml4_addr = next_page;
    next_page += PAGE_SIZE;
    let pdpt_addr = next_page;
    next_page += PAGE_SIZE;
    let pd_addr = next_page;
    next_page += PAGE_SIZE;
    let pt_addr = next_page;
    let _ = next_page + PAGE_SIZE;

    let pml4_idx = (va >> 39) & 0x1FF;
    let pdpt_idx = (va >> 30) & 0x1FF;
    let pd_idx = (va >> 21) & 0x1FF;
    let pt_idx = (va >> 12) & 0x1FF;

    // PML4 entry -> PDPT (intermediate: always P|W so we can descend)
    phys_write64(m, pml4_addr + pml4_idx * 8, (pdpt_addr >> 12) << 12 | PTE_P | PTE_W);
    // PDPT entry -> PD
    phys_write64(m, pdpt_addr + pdpt_idx * 8, (pd_addr >> 12) << 12 | PTE_P | PTE_W);
    // PD entry -> PT
    phys_write64(m, pd_addr + pd_idx * 8, (pt_addr >> 12) << 12 | PTE_P | PTE_W);
    // PT entry -> physical page (leaf: caller controls W/NX via flags)
    phys_write64(m, pt_addr + pt_idx * 8, (pa >> 12) << 12 | PTE_P | flags);

    pml4_addr
}

#[test]
fn translate_identity_mapped_page() {
    let mut m = Machine::new();

    // Map VA 0x400000 -> PA 0x400000 (identity).
    let va = 0x400000_u64;
    let pa = 0x400000_u64;
    let pml4 = build_identity_map(&mut m, va, pa, PTE_W);

    m.csrs[CSR_PTBR as usize] = pml4;
    m.csrs[CSR_PMODE as usize] = 1;

    let translated = m.translate(va, mik_emu::Access::Load).expect("translation should succeed");
    assert_eq!(translated, pa);
}

#[test]
fn translate_non_canonical_address() {
    let mut m = Machine::new();
    m.csrs[CSR_PMODE as usize] = 1;

    // VA with bit 48 set is non-canonical.
    let va = 1u64 << 48;
    let result = m.translate(va, mik_emu::Access::Load);
    assert!(result.is_err());
    let fault = result.unwrap_err();
    assert_eq!(fault.code, 4); // non-canonical
    assert_eq!(fault.va, va);
}

#[test]
fn translate_not_present() {
    let mut m = Machine::new();
    let pml4 = 0x800000;
    // PML4 is all zeros — no present entries.
    m.csrs[CSR_PTBR as usize] = pml4;
    m.csrs[CSR_PMODE as usize] = 1;

    let va = 0x400000_u64;
    let result = m.translate(va, mik_emu::Access::Load);
    assert!(result.is_err());
    let fault = result.unwrap_err();
    assert_eq!(fault.code, 1); // not present
}

#[test]
fn translate_write_violation() {
    let mut m = Machine::new();
    let va = 0x400000_u64;
    let pa = 0x400000_u64;
    // Map with P=1 but W=0 (read-only).
    let pml4 = build_identity_map(&mut m, va, pa, 0);
    m.csrs[CSR_PTBR as usize] = pml4;
    m.csrs[CSR_PMODE as usize] = 1;

    // Load should succeed.
    let load = m.translate(va, mik_emu::Access::Load);
    assert!(load.is_ok());

    // Store should fail with code 2.
    let store = m.translate(va, mik_emu::Access::Store);
    assert!(store.is_err());
    let fault = store.unwrap_err();
    assert_eq!(fault.code, 2); // write violation
}

#[test]
fn translate_nx_violation() {
    let mut m = Machine::new();
    let va = 0x400000_u64;
    let pa = 0x400000_u64;
    // Map with NX=1, W=1 (writable but not executable).
    let pml4 = build_identity_map(&mut m, va, pa, PTE_NX | PTE_W);
    m.csrs[CSR_PTBR as usize] = pml4;
    m.csrs[CSR_PMODE as usize] = 1;

    // Load should succeed.
    let load = m.translate(va, mik_emu::Access::Load);
    assert!(load.is_ok());

    // Fetch should fail with code 3.
    let fetch = m.translate(va, mik_emu::Access::Fetch);
    assert!(fetch.is_err());
    let fault = fetch.unwrap_err();
    assert_eq!(fault.code, 3); // NX violation
}

#[test]
fn translate_sets_accessed_and_dirty_bits() {
    let mut m = Machine::new();
    let va = 0x400000_u64;
    let pa = 0x400000_u64;
    let pml4 = build_identity_map(&mut m, va, pa, PTE_W);

    m.csrs[CSR_PTBR as usize] = pml4;
    m.csrs[CSR_PMODE as usize] = 1;

    // Load: should set Accessed bit (bit 3).
    m.translate(va, mik_emu::Access::Load).unwrap();

    // Walk to the leaf PTE and check A bit.
    let pml4_idx = (va >> 39) & 0x1FF;
    let pdpt_idx = (va >> 30) & 0x1FF;
    let pd_idx = (va >> 21) & 0x1FF;
    let pt_idx = (va >> 12) & 0x1FF;

    let pdpt_addr = (phys_read64(&m, pml4 + pml4_idx * 8) >> 12) << 12;
    let pd_addr = (phys_read64(&m, pdpt_addr + pdpt_idx * 8) >> 12) << 12;
    let pt_addr = (phys_read64(&m, pd_addr + pd_idx * 8) >> 12) << 12;
    let leaf_pte = phys_read64(&m, pt_addr + pt_idx * 8);

    assert!(leaf_pte & (1 << 3) != 0, "Accessed bit should be set");
    assert!(leaf_pte & (1 << 4) == 0, "Dirty bit should NOT be set after load");

    // Store: should set Dirty bit (bit 4).
    m.translate(va, mik_emu::Access::Store).unwrap();
    let leaf_pte = phys_read64(&m, pt_addr + pt_idx * 8);
    assert!(leaf_pte & (1 << 4) != 0, "Dirty bit should be set after store");
}

#[test]
fn paging_disabled_is_identity() {
    let mut m = Machine::new();
    m.csrs[CSR_PMODE as usize] = 0; // paging off

    let va = 0x123456_u64;
    let translated = m.translate(va, mik_emu::Access::Load).expect("identity translation");
    assert_eq!(translated, va);
}
