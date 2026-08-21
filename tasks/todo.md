# Mik OS Paging Todo

## Phase 1: Emulator — CSR Instructions and Page Table Walker

- [x] Task 7: Add CSR instructions (RDCSR/WRCSR/SFENCE) to the emulator
- [x] Task 8: Implement the 4-level page table walker
- [x] Task 9: Add the direct-mapped TLB
- [x] Task 10: Wire paging into memory accesses and add page-fault delivery

## Checkpoint: Emulator Paging Complete

- [x] All emulator paging tests pass
- [x] Flat-memory tests still pass

## Phase 2: Kernel — Enable Paging

- [x] Task 11: Kernel sets up an identity map and enables paging
- [ ] Task 12: Kernel handles a page fault

## Checkpoint: Paging MVP Complete

- [x] Kernel boots with paging enabled
- [ ] Page faults are handled
- [x] All tests pass
- [x] `.\run.ps1` works end-to-end
