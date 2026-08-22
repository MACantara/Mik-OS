# Implementation Plan: User Mode and System Call Round-Trip

## Overview

Add user/supervisor mode to the Mik-64 emulator and run a tiny user program through a system call. This is the smallest vertical slice of Milestone 1.2 in `ROADMAP.md`.

## Architecture Decisions

- **User mode is a `bool` on `Machine`, not a CSR:** it is a pure CPU state that changes on `SRET`, `TRAP`, and `ERET`.
- **`PTE_U` is bit 2 of a leaf PTE:** present (`PTE_P`), writable (`PTE_W`), and user (`PTE_U`) are the only flags needed for the first test.
- **Supervisor can access all pages; user can only access `PTE_U` pages:** this matches x86-64's U/S bit without adding SMAP/SMEP complexity.
- **`SRET` (opcode 0x14) enters user mode:** `pc = regs[rs1]` and `user_mode = true`. It is the counterpart to `TRAP`/`ERET`.
- **`TRAP` saves the current `user_mode` and forces supervisor; `ERET` restores it:** this gives a clean user → kernel → user round trip.
- **User program is embedded in the kernel binary:** the kernel writes it to a physical page and maps it at `0x800000` with `PTE_U`. This keeps the first test self-contained.

## Task List

### Task 1: Emulator support for user mode

**Description:** Add `user_mode` to `Machine`, add `PTE_U`, enforce it in `translate`, add `SRET`, and make `TRAP`/`ERET` save and restore `user_mode`.

**Acceptance criteria:**
- [x] `Machine` has a public `user_mode` flag that starts as `false`.
- [x] `PTE_U` (bit 2) is defined.
- [x] `translate` faults with code 5 when a user-mode access reaches a non-`PTE_U` page.
- [x] `SRET` sets `pc = regs[rs1]` and `user_mode = true`.
- [x] `TRAP` sets `previous_user_mode = user_mode` and `user_mode = false`.
- [x] `ERET` sets `pc = epc` and `user_mode = previous_user_mode`.
- [x] Existing `mik-emu` paging and trap tests still pass.

**Verification:**
- `cargo test -p mik-emu` passes.
- New or updated emulator tests verify user-mode access and the round trip.

**Dependencies:** None.

**Files likely touched:**
- `mik-emu/src/lib.rs`
- `mik-emu/tests/paging.rs`
- `mik-emu/tests/trap.rs`
- `docs/specs/mik-64.md`

**Estimated scope:** Medium.

### Task 2: Kernel user-mode binary and integration test

**Description:** Build a kernel binary that maps a user page at `0x800000`, writes a tiny user program that prints 'U' through `TRAP 1` and then halts with `TRAP 0`, and enters it with `SRET`.

**Acceptance criteria:**
- [x] `kernel_user_mode()` is added to `mik-os/src/lib.rs`.
- [x] The kernel builds a page table entry for a user page at `0x800000`.
- [x] The user program runs in user mode, calls `TRAP 1`, the handler prints 'U', and `ERET` returns to user mode.
- [x] The user program calls `TRAP 0` and the machine halts with `exit_code` 0.
- [x] `mik-os/tests/user_mode.rs` verifies the output is `!U` and paging is still enabled.

**Verification:**
- `cargo test -p mik-os --test user_mode` passes.
- `cargo test` passes.
- `run.ps1` still works.

**Dependencies:** Task 1.

**Files likely touched:**
- `mik-os/src/lib.rs`
- `mik-os/tests/user_mode.rs`

**Estimated scope:** Medium.

### Checkpoint: User Mode Works End-to-End

- [x] A user program can execute, trap to the kernel, and trap back.
- [x] Page-table permissions stop user-mode access to kernel-only pages.
- [x] All tests pass.
- [x] Changes are committed, reviewed, and merged.

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| `SRET` / `TRAP` / `ERET` mode switch bugs are subtle | High | Write a focused emulator test for the round trip before the kernel test. |
| Page-table walker changes break existing tests | Medium | Keep the supervisor path identical; only add the `PTE_U` check for `user_mode == true`. |
| Hand-assembly of the user page is error-prone | Medium | Keep the user program to three instructions and assert the output in the integration test. |

## Open Questions

- Should user-mode `TRAP` use a dedicated `ecall` instruction instead of `TRAP`? (Deferred — `TRAP` works as the system-call entry for now.)
