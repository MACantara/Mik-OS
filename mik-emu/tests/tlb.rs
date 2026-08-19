use mik_emu::{Machine, CSR_PTBR, CSR_PMODE};

const PAGE_SIZE: u64 = 4096;
const PTE_P: u64 = 1 << 0;
const PTE_W: u64 = 1 << 1;
const PTE_A: u64 = 1 << 3;

fn phys_write64(m: &mut Machine, addr: u64, val: u64) {
    let bytes = val.to_le_bytes();
    m.mem[addr as usize..addr as usize + 8].copy_from_slice(&bytes);
}

fn phys_read64(m: &Machine, addr: u64) -> u64 {
    let bytes = &m.mem[addr as usize..addr as usize + 8];
    u64::from_le_bytes(bytes.try_into().unwrap())
}

fn build_identity_map(m: &mut Machine, va: u64, pa: u64, flags: u64) -> u64 {
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

    phys_write64(m, pml4_addr + pml4_idx * 8, (pdpt_addr >> 12) << 12 | PTE_P | PTE_W);
    phys_write64(m, pdpt_addr + pdpt_idx * 8, (pd_addr >> 12) << 12 | PTE_P | PTE_W);
    phys_write64(m, pd_addr + pd_idx * 8, (pt_addr >> 12) << 12 | PTE_P | PTE_W);
    phys_write64(m, pt_addr + pt_idx * 8, (pa >> 12) << 12 | PTE_P | flags);

    pml4_addr
}

#[test]
fn tlb_hit_uses_cached_translation() {
    let mut m = Machine::new();
    let va = 0x400000_u64;
    let pa = 0x400000_u64;
    let pml4 = build_identity_map(&mut m, va, pa, PTE_W);

    m.csrs[CSR_PTBR as usize] = pml4;
    m.csrs[CSR_PMODE as usize] = 1;

    // First translation: walks the page table, fills TLB.
    let result1 = m.translate(va, mik_emu::Access::Load).unwrap();
    assert_eq!(result1, pa);

    // Change the PTE to point to a different physical page.
    let pml4_idx = (va >> 39) & 0x1FF;
    let pdpt_idx = (va >> 30) & 0x1FF;
    let pd_idx = (va >> 21) & 0x1FF;
    let pt_idx = (va >> 12) & 0x1FF;
    let pdpt_addr = (phys_read64(&m, pml4 + pml4_idx * 8) >> 12) << 12;
    let pd_addr = (phys_read64(&m, pdpt_addr + pdpt_idx * 8) >> 12) << 12;
    let pt_addr = (phys_read64(&m, pd_addr + pd_idx * 8) >> 12) << 12;

    // Remap to a different physical address (0x500000).
    let new_pa = 0x500000_u64;
    phys_write64(&mut m, pt_addr + pt_idx * 8, (new_pa >> 12) << 12 | PTE_P | PTE_W);

    // Second translation: should hit the TLB and return the OLD mapping,
    // not the new one, proving the TLB is being used.
    let result2 = m.translate(va, mik_emu::Access::Load).unwrap();
    assert_eq!(
        result2, pa,
        "TLB hit should return cached mapping, not the updated PTE"
    );

    // After flush, the new mapping should be visible.
    m.flush_tlb();
    let result3 = m.translate(va, mik_emu::Access::Load).unwrap();
    assert_eq!(result3, new_pa, "After TLB flush, new mapping should be used");
}

#[test]
fn sfence_flushes_tlb() {
    let mut m = Machine::new();
    let va = 0x400000_u64;
    let pa = 0x400000_u64;
    let pml4 = build_identity_map(&mut m, va, pa, PTE_W);

    m.csrs[CSR_PTBR as usize] = pml4;
    m.csrs[CSR_PMODE as usize] = 1;

    // First translation fills the TLB.
    m.translate(va, mik_emu::Access::Load).unwrap();

    // Clear the A bit.
    let pt_idx = (va >> 12) & 0x1FF;
    let pml4_idx = (va >> 39) & 0x1FF;
    let pdpt_idx = (va >> 30) & 0x1FF;
    let pd_idx = (va >> 21) & 0x1FF;
    let pdpt_addr = (phys_read64(&m, pml4 + pml4_idx * 8) >> 12) << 12;
    let pd_addr = (phys_read64(&m, pdpt_addr + pdpt_idx * 8) >> 12) << 12;
    let pt_addr = (phys_read64(&m, pd_addr + pd_idx * 8) >> 12) << 12;
    let pte = phys_read64(&m, pt_addr + pt_idx * 8);
    phys_write64(&mut m, pt_addr + pt_idx * 8, pte & !PTE_A);

    // Flush the TLB.
    m.flush_tlb();

    // Next translation should walk the page table again and re-set A bit.
    m.translate(va, mik_emu::Access::Load).unwrap();
    let pte = phys_read64(&m, pt_addr + pt_idx * 8);
    assert!(pte & PTE_A != 0, "A bit should be re-set after TLB flush + re-walk");
}
