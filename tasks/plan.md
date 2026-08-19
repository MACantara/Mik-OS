# Implementation Plan: Mik OS MVP

## Overview

Build the first working version of Mik OS by first creating a simplified but x86-64-like virtual machine called Mik-64 and a Rust interpreter/emulator for it. The goal is to learn OS internals in a clean environment and have a real booting system before touching real x86-64 hardware.

## Architecture Decisions

- **Language:** Rust. It is the user’s preferred language and provides strong control over memory layout and `no_std` kernel code. Rust is used for the emulator now and for the real x86-64 Mik OS later.
- **Custom VM first (Mik-64):** Mik-64 is intentionally close to x86-64 in *concepts* (64-bit, flat physical memory, serial MMIO, simple interrupts) so the later transition is about legacy, not a total rewrite. Because Mik-64 is a custom ISA, the kernel that runs on it in the MVP must either be hand-assembled or the VM must be changed to a Rust-supported architecture (e.g. RISC-V 64).
- **Flat physical memory first:** The MVP uses a single, flat physical address space. Page tables and virtual memory are introduced later, after the kernel already works.
- **Simple `match`-based interpreter emulator:** The first emulator decodes and executes one instruction at a time. Faster or more advanced designs (function tables, JIT) are deferred until they are needed.
- **Flat binary boot protocol:** The emulator loads the kernel flat binary at a fixed address, sets a documented initial register state, and starts execution. No real boot sector or firmware is emulated in the MVP.
- **Hand-assembled user program first:** The first user-space test is a tiny hand-assembled binary. A Rust `no_std` program compiled for Mik-64 is added once the syscall path is proven.

## Task List

### Phase 1: Mik-64 Machine and Emulator

#### Task 1: Write the Mik-64 machine specification

**Description:** Define the Mik-64 machine: register set, instruction set, memory map, boot protocol, memory-mapped serial I/O, and a minimal interrupt/syscall model.

**Acceptance criteria:**
- [ ] The spec lives in `docs/specs/mik-64.md` and is readable end-to-end.
- [ ] It defines the register set, instruction set, and instruction encoding.
- [ ] It defines the flat physical memory map and serial device address.
- [ ] It defines the exact initial machine state after boot.
- [ ] It is small enough that the emulator can be written in one focused session.

**Verification:**
- [ ] A reader can hand-assemble a "Hello, Mik!" program from the spec.
- [ ] The spec file is committed (or ready for commit) with the project.

**Dependencies:** None

**Files likely touched:**
- `docs/specs/mik-64.md`

**Estimated scope:** Small

#### Task 2: Build the Mik-64 emulator in Rust

**Description:** Create a `mik-emu` Rust crate that loads a flat binary, decodes and executes instructions according to the spec, and writes to a serial console.

**Acceptance criteria:**
- [ ] `mik-emu` builds with `cargo build`.
- [ ] It loads a flat binary from a CLI argument.
- [ ] It can run the hand-assembled "Hello, Mik!" test program and print the expected output.
- [ ] It halts cleanly and exits with status 0.

**Verification:**
- [ ] Run `cargo build` in `mik-emu/`.
- [ ] Run `mik-emu` with `tests/hello_mik.bin` and see `Hello, Mik!\n` on stdout.

**Dependencies:** Task 1

**Files likely touched:**
- `mik-emu/Cargo.toml`
- `mik-emu/src/main.rs`
- `mik-emu/tests/hello_mik.bin` or `mik-emu/tests/hello_mik.S`

**Estimated scope:** Medium

#### Task 3: Write the Mik OS kernel boot path

**Description:** Build the smallest Mik OS kernel that the emulator can load. Because Mik-64 is a custom ISA, the kernel is hand-assembled in the `mik-os` crate using `mik_emu::encode`. It boots, sets up a minimal stack, and prints `Mik OS` through the emulator's serial device.

**Acceptance criteria:**
- [ ] The `mik-os` crate produces a flat binary (using `mik_emu::encode`).
- [ ] The flat binary is loaded by `mik-emu`.
- [ ] On boot, it prints `Mik OS` to the serial console.
- [ ] A top-level build/run command builds and runs the OS.

**Verification:**
- [ ] Run the top-level build command and see `Mik OS` printed.
- [ ] The emulator exits cleanly.

**Dependencies:** Task 2

**Files likely touched:**
- `mik-os/Cargo.toml`
- `mik-os/src/main.rs`
- `mik-os/src/boot.S` (optional)
- `mik-os/mik-64-target.json` (optional, only if needed)
- `mik-os/linker.ld` (optional)
- `run.ps1` / `Justfile` / `Makefile`

**Estimated scope:** Medium

### Phase 2: Core OS Concepts

#### Task 4: Add a simple physical memory manager

**Description:** Implement a basic physical memory allocator. In the flat memory model, this can be a bump allocator or a bitmap over fixed-size physical pages.

**Acceptance criteria:**
- [x] A physical memory allocator exists in `mik-os/src/lib.rs` (bump allocator over 4 KiB pages).
- [x] The kernel can allocate and free one or more blocks.
- [x] A test demonstrates allocation and deallocation.

**Verification:**
- [ ] A kernel test or a hand-assembled user test runs and reports success.

**Dependencies:** Task 3

**Files likely touched:**
- `mik-os/src/memory.rs`
- `mik-os/src/lib.rs`

**Estimated scope:** Small

#### Task 5: Implement interrupts and a system call

**Description:** Add a `TRAP` (or `SYSCALL`) instruction to the Mik-64 spec and emulator. Mik OS sets up a simple trap vector, handles a `print char` system call, and returns.

**Acceptance criteria:**
- [x] The Mik-64 spec includes a trap/syscall model.
- [x] `mik-emu` can enter a kernel trap handler and return.
- [x] Mik OS has a system-call handler that prints a character.
- [x] A test user program calls the syscall and produces output.

**Verification:**
- [ ] Run the user test program and see expected serial output.

**Dependencies:** Task 4

**Files likely touched:**
- `docs/specs/mik-64.md` (updated with TRAP)
- `mik-emu/src/main.rs`
- `mik-os/src/interrupts.rs`
- `mik-os/src/syscall.rs`
- `mik-os/tests/syscall_test.S`

**Estimated scope:** Medium

### Phase 3: Integration

#### Task 6: Add a build/run convenience script

**Description:** A single command builds `mik-emu` and `mik-os` and runs Mik OS in the emulator.

**Acceptance criteria:**
- [x] One command (e.g. `cargo run`, `make run`, or `.\run.ps1`) builds and runs the OS.
- [x] The command works from a clean checkout and reports clear errors.

**Verification:**
- [ ] Run the command from a clean state and see `Mik OS`.

**Dependencies:** Task 3 (can be finalized after Task 5)

**Files likely touched:**
- `run.ps1`
- `Justfile`
- `Makefile`

**Estimated scope:** Small

### Checkpoint: MVP Complete

- [x] One command boots Mik OS in the Mik-64 emulator.
- [x] The OS prints to serial, allocates memory, and handles a basic syscall.
- [x] All six tasks pass their acceptance criteria.

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Custom Rust target and linker script are tricky for a first OS | Medium | Start with a flat binary and a tiny assembly stub; add a custom target only when the build really needs it. |
| The emulator becomes its own rabbit hole | High | Keep the instruction set tiny; add instructions only when the OS needs them. |
| Loss of momentum | High | Every task ends with a visible, runnable output. |

## Open Questions

- **Resolved:** The boot protocol is in `docs/specs/mik-64.md` — load at `0x400000`, initial `pc` = `0x400000`, `x15` = top of RAM.
- **Resolved:** The first Mik-64 instruction set has 15 instructions (opcodes `0x00`..`0x0E`).
- **Resolved:** The test program embeds its data at the end of the flat binary.
- **Resolved:** The Mik-64 kernel is hand-assembled using `mik_emu::encode` in the `mik-os` crate.

Remaining:
- Which additional instructions are needed for subroutines, a physical memory manager, and a `TRAP`/`SYSCALL` model?
- Where should the bump allocator keep its `next_page` counter in memory?
- Should the `TRAP` vector be a single fixed address, a table indexed by syscall number, or a CSR-like register?
