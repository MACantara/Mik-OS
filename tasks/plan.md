# Implementation Plan: User Mode and System Call Round-Trip

## Overview

Add user/supervisor mode to the Mik-64 emulator and run a tiny user program through a system call. This is the smallest vertical slice of Milestone 1.2 in `ROADMAP.md`.

## Architecture Decisions

- **User mode is a `bool` on `Machine`, not a CSR:** it is a pure CPU state that changes on `SRET`, `TRAP`, and `ERET`.
- **`PTE_U` is bit 2 of a leaf PTE:** present (`PTE_P`), writable (`PTE_W`), and user (`PTE_U`) are the only flags needed for the first test.
- **Supervisor can access all pages; user can only access `PTE_U` pages:** this matches x86-64's U/S bit without adding SMAP/SMEP complexity.
- **`SRET` (opcode 0x14) enters user mode:** `pc = regs[rs1]` and `user_mode = true`. It is the counterpart to `TRAP`/`ERET`.
- **`TRAP` saves the current `user_mode` and forces supervisor; `ERET` restores it:** this gives a clean user -> kernel -> user round trip.
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

- Should user-mode `TRAP` use a dedicated `ecall` instruction instead of `TRAP`? (Deferred -- `TRAP` works as the system-call entry for now.)

# Implementation Plan: Timer and Interrupts (Milestone 1.3 Slice)

## Overview

Add a programmable interval timer and the `INT`/`IRET` instructions so the kernel can receive periodic timer ticks while running a user program. This is the smallest vertical slice of Milestone 1.3 and leaves the full round-robin scheduler for the next slice.

## Architecture Decisions

- `CSR_TIMER` (CSR 2) stores the interval; writing it reloads both the interval and the down counter.
- The timer counter decrements once per emulated instruction; when it reaches zero the machine raises a pending interrupt.
- The timer interrupt vector is fixed at `0x2020`, read from physical memory when the interrupt is taken.
- `INT` (0x15) and `IRET` (0x16) mirror `TRAP`/`ERET` but use the `0x2020` interrupt vector; `IRET` restores `pc` and `user_mode`.
- Timer delivery is not re-entrant for the first slice; the kernel is expected to keep the handler short and the interval long enough to avoid nested ticks.

## Task List

### Task 1: Emulator timer and `INT`/`IRET`

**Description:** Add `timer_counter`, `timer_interval`, and `pending_interrupt` to `Machine`; handle `WRCSR`/`RDCSR` for `CSR_TIMER`; add `0x15` `INT` and `0x16` `IRET`; and deliver the timer tick through `0x2020`.

**Acceptance criteria:**
- [x] `Machine` has a programmable down counter and interval.
- [x] Writing `CSR_TIMER` reloads both counter and interval.
- [x] Timer expiry sets a pending flag; the next `step` jumps to `mem64[0x2020]` and forces supervisor mode.
- [x] `INT` jumps to `mem64[0x2020]` and saves `previous_user_mode`.
- [x] `IRET` sets `pc = epc` and `user_mode = previous_user_mode`.
- [x] `mik-emu/tests/timer.rs` shows a timer tick, an `INT`/`IRET` round trip, and a `kernel_timer` tick.

**Verification:**
- `cargo test -p mik-emu --test timer` passes.

**Dependencies:** None.

**Files likely touched:**
- `mik-emu/src/lib.rs`
- `mik-emu/tests/timer.rs`
- `docs/specs/mik-64.md`
- `docs/specs/mik-64-paging.md`

**Estimated scope:** Small.

### Task 2: Kernel `kernel_timer` binary and test

**Description:** Build a hand-assembled kernel that sets the timer, `SRET`s into a user program, and lets a timer handler print 'T' and `IRET` back; after three ticks the handler halts.

**Acceptance criteria:**
- [x] `kernel_timer()` is added to `mik-os/src/lib.rs`.
- [x] The kernel installs a `0x2020` interrupt handler.
- [x] A user program that `JMP 0` loops until the timer tick fires.
- [x] The timer handler prints 'T' and `IRET`s; after three ticks the machine halts.
- [x] `mik-os/tests/timer.rs` verifies output `!TTT` and `exit_code == 0`.

**Verification:**
- `cargo test -p mik-os --test timer` passes.
- `cargo test` still passes.
- `run.ps1` still works.

**Dependencies:** Task 1.

**Files likely touched:**
- `mik-os/src/lib.rs`
- `mik-os/tests/timer.rs`

**Estimated scope:** Small.

### Checkpoint: Timer and Interrupts Work End-to-End

- [x] The emulator can deliver a periodic timer tick to a supervisor handler.
- [x] A user program can run and be interrupted, then resume with `IRET`.
- [x] All tests pass.
- [x] Changes are committed, reviewed, and merged.
- [x] `ROADMAP.md` is updated to show Milestone 1.3 in progress.

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Timer delivery during a previous interrupt corrupts `previous_user_mode` | High | Keep the first test's interval large enough that the handler finishes before the next tick. |
| Hand-assembly of the `kernel_timer` binary is error-prone | Medium | Reuse the `kernel_user_mode` page-table setup and keep the handler short. |
| Timer interaction with paging breaks existing tests | Medium | Timer vector is read from physical memory and the handler is identity-mapped. |

# Implementation Plan: Tiny Mik-64 Assembler (Milestone 1.4 Slice)

## Overview

Add a tiny text-to-binary assembler for Mik-64 so user programs can be written in a minimal line-oriented syntax instead of hand-encoded words. This slice covers the assembler tool and a single `Hello` user program test; `exec()` and a pseudo file system are deferred.

## Architecture Decisions

- The assembler lives in a new `mik-asm` workspace crate so it can be built and run as `mik-asm <in.s> <out.bin>`.
- Syntax is line-oriented, space/comma separated tokens, `#` line comments, `label:` definitions, and a `.string` directive.
- Two-pass assembly: first pass collects instructions and data items; second pass resolves labels and emits a flat binary with code followed by data.
- Only the instructions needed for the first test are required, but all 23 Mik-64 opcodes are mapped for completeness.
- The `Hello` program uses the memory-mapped serial port directly and `HALT`; it does not rely on a `TRAP` handler, keeping the test self-contained.

## Task List

### Task 1: Create the `mik-asm` crate and parser

**Description:** Add `mik-asm` to the workspace with a library `assemble(src, base)` and a `mik-asm` binary. Support mnemonics, registers, immediate literals, labels, and `.string`.

**Acceptance criteria:**
- [x] `mik-asm/Cargo.toml` exists and `mik-asm` is in the workspace members.
- [x] `mik-asm/src/lib.rs` parses source and returns a `Vec<u8>` or a clear error.
- [x] `mik-asm/src/main.rs` reads an input file, assembles it, and writes the output file.
- [x] All existing Mik-64 opcodes are mapped to mnemonics.

**Verification:**
- `cargo build -p mik-asm` succeeds.
- `cargo test -p mik-asm` passes.

**Dependencies:** None.

**Files likely touched:**
- `Cargo.toml`
- `mik-asm/Cargo.toml`
- `mik-asm/src/lib.rs`
- `mik-asm/src/main.rs`

**Estimated scope:** Medium.

### Task 2: Hello-world user program and integration test

**Description:** Add a `mik-asm/tests/assembler.rs` test with a source that prints `Hello` and halts, then runs it under `mik_emu` and asserts the output.

**Acceptance criteria:**
- [x] A `.s` source assembles to a binary that prints `Hello\n` (or `Hello\n`).
- [x] `cargo test -p mik-asm` runs the assembled program and verifies the output.
- [x] `cargo test` across the workspace still passes.
- [x] `run.ps1` still works.

**Verification:**
- `cargo test` passes.

**Dependencies:** Task 1.

**Files likely touched:**
- `mik-asm/tests/assembler.rs`

**Estimated scope:** Small.

### Checkpoint: Assembler Works End-to-End

- [x] `mik-asm` can be built and run from the command line.
- [x] A hand-written `.s` file produces a working Mik-64 binary.
- [x] All tests pass.
- [x] Changes are committed, reviewed, and merged.
- [x] `ROADMAP.md` is updated to show Milestone 1.4 in progress.

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Two-pass label resolution has off-by-one errors in branch offsets | High | Keep the test simple with only a few labels and assert exact output. |
| New crate workspace integration breaks existing builds | Medium | Build the full workspace and run all tests after adding `mik-asm`. |
| Over-engineering a full assembler | Medium | Support only the syntax needed for the first test; don't add macros or complex directives. |
