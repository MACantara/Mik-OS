# Mik OS Paging Todo

## Phase 1: Emulator — CSR Instructions and Page Table Walker

- [ ] Task 7: Add CSR instructions (RDCSR/WRCSR/SFENCE) to the emulator
- [ ] Task 8: Implement the 4-level page table walker
- [ ] Task 9: Add the direct-mapped TLB
- [ ] Task 10: Wire paging into memory accesses and add page-fault delivery

## Checkpoint: Emulator Paging Complete

- [ ] All emulator paging tests pass
- [ ] Flat-memory tests still pass

## Phase 2: Kernel — Enable Paging

- [ ] Task 11: Kernel sets up an identity map and enables paging
- [ ] Task 12: Kernel handles a page fault

## Checkpoint: Paging MVP Complete

- [ ] Kernel boots with paging enabled
- [ ] Page faults are handled
- [ ] All tests pass
- [ ] `.\run.ps1` works end-to-end
