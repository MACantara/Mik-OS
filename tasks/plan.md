# Implementation Plan: Mik-64 Paging / Virtual Memory

## Overview

Add 4-level paging with 4 KiB pages to the Mik-64 virtual machine, matching the
x86-64 model. The kernel will set up an identity map, enable paging via CSR
instructions, handle page faults, and continue to boot. This builds on the
existing flat-memory MVP and prepares for the real x86-64 port.

## Architecture Decisions

- **4 KiB pages** — matches x86-64 and the existing bump allocator.
- **4-level page table** (PML4 → PDPT → PD → PT) — 48-bit VAs, 9 bits per level.
- **CSR instructions** (`RDCSR`/`WRCSR`/`SFENCE`) — closest to real hardware
  (x86 CR3, RISC-V SATP). PTBR and PMODE are the first two CSRs.
- **Paging switched on later** — boot flat, kernel builds page tables, then
  enables paging. More realistic and easier to debug than always-on.
- **Simple direct-mapped TLB** (16 entries) — teaches the TLB concept without
  complex replacement policies. Flushed on SFENCE and PTBR writes.
- **Page-fault vector at 0x2010** — separate from the syscall vector at 0x2000.
  Fault code in x10, faulting VA in x11, ERET retries the faulting instruction.

## Task List

### Phase 1: Emulator — CSR Instructions and Page Table Walker

#### Task 7: Add CSR instructions to the emulator

**Description:** Add `RDCSR` (0x11), `WRCSR` (0x12), and `SFENCE` (0x13) to the
emulator. Add a `csrs: [u64; 256]` array to `Machine`. CSR 0 = PTBR, CSR 1 =
PMODE. `SFENCE` flushes the TLB (a no-op until the TLB exists, but the
instruction must be accepted).

**Acceptance criteria:**
- [ ] `RDCSR` reads a CSR value into a register.
- [ ] `WRCSR` writes a register value into a CSR.
- [ ] `SFENCE` is accepted without error.
- [ ] Integration test verifies read/write of PTBR and PMODE.

**Verification:**
- [ ] `cargo test -p mik-emu --test csr` passes.

**Dependencies:** None (builds on existing emulator).

**Files likely touched:**
- `mik-emu/src/lib.rs`
- `mik-emu/tests/csr.rs`
- `docs/specs/mik-64.md`

**Estimated scope:** Small

#### Task 8: Implement the page table walker

**Description:** Add a `translate(va, access_type) -> Result<u64, PageFault>`
function to `Machine` that walks the 4-level page table when `PMODE = 1`. When
`PMODE = 0`, return the address unchanged. Handle all fault codes (not present,
write violation, NX violation, non-canonical). Set A/D bits on access.

**Acceptance criteria:**
- [ ] With `PMODE = 0`, translation is identity (existing behavior preserved).
- [ ] With `PMODE = 1`, a valid page table produces the correct physical address.
- [ ] Missing PTE raises page fault code 1.
- [ ] Write to read-only page raises page fault code 2.
- [ ] Fetch from NX page raises page fault code 3.
- [ ] Non-canonical VA raises page fault code 4.
- [ ] A/D bits are set in the PTE on access.

**Verification:**
- [ ] `cargo test -p mik-emu --test paging` passes.

**Dependencies:** Task 7

**Files likely touched:**
- `mik-emu/src/lib.rs`
- `mik-emu/tests/paging.rs`
- `docs/specs/mik-64.md`

**Estimated scope:** Medium

#### Task 9: Add the TLB

**Description:** Add a 16-entry direct-mapped TLB to `Machine`. Index by
`va[15:12]`. On a translate, check the TLB first; on miss, walk the page table
and fill the TLB. Flush on `SFENCE`, on `WRCSR PTBR`, and on `WRCSR PMODE`.

**Acceptance criteria:**
- [ ] TLB hit returns the cached translation without walking the page table.
- [ ] TLB miss walks the page table and fills the entry.
- [ ] `SFENCE` flushes all TLB entries.
- [ ] `WRCSR PTBR` flushes all TLB entries.
- [ ] Test verifies TLB hit by checking PTE A bit is not re-set on second access.

**Verification:**
- [ ] `cargo test -p mik-emu --test tlb` passes.

**Dependencies:** Task 8

**Files likely touched:**
- `mik-emu/src/lib.rs`
- `mik-emu/tests/tlb.rs`

**Estimated scope:** Small

#### Task 10: Wire paging into memory accesses and add page-fault delivery

**Description:** Route all instruction fetches, loads, and stores through
`translate()` when `PMODE = 1`. On a page fault, save `epc = faulting PC`,
set `x10 = fault_code`, `x11 = faulting VA`, and jump to `mem64[0x2010]`.

**Acceptance criteria:**
- [ ] Instruction fetch through paging works.
- [ ] Load/store through paging works.
- [ ] Page fault jumps to the handler at `mem64[0x2010]`.
- [ ] `ERET` retries the faulting instruction.
- [ ] Existing flat-memory tests still pass (PMODE = 0 path).

**Verification:**
- [ ] `cargo test` passes (all existing + new tests).

**Dependencies:** Task 9

**Files likely touched:**
- `mik-emu/src/lib.rs`
- `mik-emu/tests/pagefault.rs`

**Estimated scope:** Medium

### Checkpoint: Emulator Paging Complete

- [ ] All emulator paging tests pass.
- [ ] Flat-memory tests still pass.
- [ ] Ready to update the kernel.

### Phase 2: Kernel — Enable Paging

#### Task 11: Kernel sets up an identity map and enables paging

**Description:** Update the hand-assembled kernel to build a 4-level identity
map for its code/data region, install a page-fault handler at `0x2010`, set
`PTBR`, and enable `PMODE`. The kernel should continue to print its boot
message after paging is enabled.

**Acceptance criteria:**
- [ ] Kernel builds page tables in physical memory.
- [ ] Kernel sets PTBR to the PML4 root.
- [ ] Kernel enables paging (PMODE = 1).
- [ ] Kernel prints its boot message after paging is on.
- [ ] `.\run.ps1` produces the expected output.

**Verification:**
- [ ] `cargo test -p mik-os` passes.
- [ ] `.\run.ps1` prints the boot message.

**Dependencies:** Task 10

**Files likely touched:**
- `mik-os/src/lib.rs`
- `mik-os/tests/kernel_boot.rs`

**Estimated scope:** Medium

#### Task 12: Kernel handles a page fault

**Description:** Add a page-fault handler to the kernel that prints a diagnostic
message and halts. Add a deliberate page fault (e.g., access an unmapped VA) to
demonstrate the handler runs.

**Acceptance criteria:**
- [ ] Page-fault handler is installed at `0x2010`.
- [ ] A deliberate access to an unmapped VA triggers the handler.
- [ ] The handler prints a fault message and halts.
- [ ] Integration test verifies the fault output.

**Verification:**
- [ ] `cargo test -p mik-os --test pagefault` passes.

**Dependencies:** Task 11

**Files likely touched:**
- `mik-os/src/lib.rs`
- `mik-os/tests/pagefault.rs`

**Estimated scope:** Small

### Checkpoint: Paging MVP Complete

- [ ] Kernel boots with paging enabled.
- [ ] Page faults are handled.
- [ ] All tests pass.
- [ ] `.\run.ps1` works end-to-end.

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Page table walk bugs are hard to debug | High | Write walker unit tests with hand-crafted page tables before wiring into the CPU. |
| Breaking existing flat-memory tests | Medium | Always check PMODE before translating; flat path is identity. Run full test suite after each task. |
| Kernel page table setup is complex in hand-assembly | Medium | Keep the identity map small (only the pages the kernel uses). Add comments mapping each instruction to the assembly. |
| TLB correctness | Low | TLB is a pure cache; disabling it (flush on every access) must still be correct. Test with and without TLB. |

## Open Questions

- Should we add a global bit for kernel pages? (Deferred — one address space.)
- Should we add user/kernel privilege enforcement? (Deferred — no user mode yet.)
