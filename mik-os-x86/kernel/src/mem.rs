//! Physical memory: a free-list frame allocator fed by the E820 map, plus the
//! kernel's own page-table management.
//!
//! The allocator's free list stores each free frame's link in the frame's
//! first 8 bytes — no backing structure, no size cap. This is the same
//! progression Phase 1's Mik-64 kernel followed (bump -> free list).

use crate::e820::{self, E820Entry};

pub const PTE_P: u64 = 1 << 0;
pub const PTE_W: u64 = 1 << 1;
pub const PTE_U: u64 = 1 << 2;
pub const PTE_PS: u64 = 1 << 7;

extern "C" {
    // Boot-time page tables living in .bss — already identity-mapped, so they
    // are safe to write before the new map is active.
    static mut pml4: [u64; 512];
    static mut pd: [u64; 512];
    static __bss_end: u8;
}

static mut FREE_HEAD: u64 = 0;

/// Push a 4 KiB frame back onto the free list.
///
/// Safety: `pa` must be a frame the allocator previously handed out (or one
/// being seeded during `init`), and must be identity-mapped writable.
pub unsafe fn free_frame(pa: u64) {
    *(pa as *mut u64) = FREE_HEAD;
    FREE_HEAD = pa;
}

/// Pop a 4 KiB frame off the free list, or `None` if empty.
pub unsafe fn alloc_frame() -> Option<u64> {
    let head = FREE_HEAD;
    if head == 0 {
        None
    } else {
        FREE_HEAD = *(head as *const u64);
        Some(head)
    }
}

fn kernel_end() -> u64 {
    let end = unsafe { &__bss_end as *const u8 as u64 };
    (end + 0xFFF) & !0xFFF
}

/// Seed the free list from the E820 map and return the usable frame count.
///
/// Two exclusion ranges keep the allocator honest:
/// - everything below 1 MiB: IVT/BDA, the boot sector, the E820 buffer, and
///   the stage2 load blob all live there;
/// - the kernel image `0x400000..__bss_end`: code, page tables, stack.
pub unsafe fn init() -> u64 {
    let mut map = [E820Entry { base: 0, len: 0, typ: 0, acpi: 0 }; 32];
    let n = e820::read_map(&mut map);
    let kend = kernel_end();
    let mut frames = 0u64;
    for e in &map[..n] {
        if !e.usable() {
            continue;
        }
        let mut f = (e.base + 0xFFF) & !0xFFF;
        while f + 0x1000 <= e.base + e.len {
            let below_1mib = f < 0x10_0000;
            let in_kernel = f >= 0x40_0000 && f < kend;
            if !below_1mib && !in_kernel {
                free_frame(f);
                frames += 1;
            }
            f += 0x1000;
        }
    }
    frames
}

/// Extend the boot page tables to a full 1 GiB identity map and reload CR3.
///
/// boot.S built entries pd[0..3] (6 MiB) to survive the mode switch; now that
/// Rust runs, the map grows to cover all RAM the E820 map can report. Writing
/// CR3 also flushes the TLB, which is what makes the new entries visible.
pub unsafe fn extend_identity_map() {
    let pd_ptr = core::ptr::addr_of_mut!(pd);
    for (i, e) in (*pd_ptr).iter_mut().enumerate().skip(3) {
        *e = (i as u64) * 0x20_0000 | PTE_P | PTE_W | PTE_PS;
    }
    core::arch::asm!("mov cr3, {}", in(reg) core::ptr::addr_of!(pml4) as u64);
}

/// Load `p4` (physical address of a PML4) into CR3 — the x86-64 context
/// switch for address spaces. Also flushes the TLB (no PCID in use).
pub unsafe fn switch_cr3(p4: u64) {
    core::arch::asm!("mov cr3, {}", in(reg) p4);
}

/// Map `pa` at `va` (both 4 KiB aligned) in the address space rooted at `p4`.
/// Missing intermediate tables are allocated and zeroed. User pages set
/// `PTE_U` on the whole chain — every level must carry it for ring 3.
pub unsafe fn map_4k(p4: *mut u64, va: u64, pa: u64, flags: u64) {
    let user = flags & PTE_U;
    let mut table = p4;
    for shift in [39u32, 30, 21] {
        let idx = (va >> shift) as usize & 0x1FF;
        let e = &mut *table.add(idx);
        if *e & PTE_P == 0 {
            let next = alloc_frame().expect("out of frames for page tables");
            core::ptr::write_bytes(next as *mut u8, 0, 4096);
            *e = next | PTE_P | PTE_W | user;
        }
        table = (*e & !0xFFF) as *mut u64;
    }
    *table.add((va >> 12) as usize & 0x1FF) = pa | flags;
}

/// Walk `p4` to the leaf PTE for `va` without allocating anything. Returns a
/// raw pointer to the PTE, or null if any intermediate level is missing.
/// Used by the page-fault handler to inspect the faulting page's flags.
pub unsafe fn find_pte(p4: *const u64, va: u64) -> *mut u64 {
    let mut table = p4;
    for shift in [39u32, 30, 21] {
        let e = *table.add((va >> shift) as usize & 0x1FF);
        if e & PTE_P == 0 {
            return core::ptr::null_mut();
        }
        table = (e & !0xFFF) as *const u64;
    }
    table.add((va >> 12) as usize & 0x1FF) as *mut u64
}

/// Build a second address space: a fresh PML4 whose PDPT shares the kernel's
/// identity PD (low 1 GiB — kernel code, stack, and allocator all keep
/// working after the switch) while PDPT slot 1 points at private user tables
/// covering 0x40000000..0x80000000. This is the Mik-64 shape on real
/// hardware: shared kernel mappings, private user region.
///
/// PTE_U semantics: ring 3 may cross a level only if that level's entry has
/// U set. `up4[0]` carries U because user pages hang under it; `updpt[0]`
/// (the shared kernel PD) does NOT — so user mode still can't touch kernel
/// memory even though it shares the table.
pub unsafe fn build_user_table() -> u64 {
    let up4 = alloc_frame().expect("out of frames for pml4");
    let updpt = alloc_frame().expect("out of frames for pdpt");
    core::ptr::write_bytes(up4 as *mut u8, 0, 4096);
    core::ptr::write_bytes(updpt as *mut u8, 0, 4096);
    *(up4 as *mut u64) = updpt | PTE_P | PTE_W | PTE_U;
    *(updpt as *mut u64) = core::ptr::addr_of!(pd) as u64 | PTE_P | PTE_W;
    up4
}
