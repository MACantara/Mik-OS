# Mik OS Roadmap

This roadmap maps the path from the current Mik-64 learning emulator to a real x86-64 operating system running under QEMU or Bochs. It is intentionally educational: each phase is designed to teach one major OS concept before the next one is added.

## Goal

Build a from-scratch operating system named Mik OS and deeply understand its internals. The approach is to first explore the ideas in a simplified, self-built virtual machine and emulator, then port the lessons to real x86-64 hardware.

## Current State

Mik OS currently runs on the Mik-64 virtual machine inside a Rust emulator. The hand-assembled kernel demonstrates the following working concepts:

- Boot, memory-mapped serial I/O, and a simple `HALT` / `print_char` syscall model.
- A bump page allocator tracking the next free 4 KiB page at `0x700000`.
- Four-level paging with 4 KiB pages, a 16-entry direct-mapped TLB, and CSR-style `PTBR` / `PMODE` controls.
- An identity-mapped kernel page table and a kernel page-fault handler that prints `F<fault_code>`.
- User/supervisor mode with `SRET`, `PTE_U`, and a user-mode system call round-trip.
- A programmable interval timer, `INT`/`IRET`, and a user program that receives timer ticks.
- A tiny text assembler (`mik-asm`) that produces a flat Mik-64 binary from a minimal line-oriented syntax.
- End-to-end build and run via `cargo test` and `run.ps1`.

The paging/VM, user-mode, timer, and assembler milestones are complete.
Phase 2 has started: Milestone 2.1 is in progress — a minimal x86-64 long-mode
kernel boots under QEMU via the PVH direct-boot ABI, sets up a GDT and initial
page tables, and prints a serial banner.

## Phase 1: Mik-64 OS Core (Complete Learning Sandbox)

The first objective is to prove every major OS concept on the safe, inspectable Mik-64 emulator before real hardware complicates things.

### Milestone 1.1 — Memory Management Beyond Bump Allocation

**Goal:** Move from a one-way bump allocator to a richer physical and virtual memory manager.

- Implement a free-list / buddy allocator for physical pages.
- Add page-table allocation helpers (`pt_alloc`, `pt_free`).
- Support separate per-process page tables (not just the kernel identity map).
- Add demand paging: the page-fault handler allocates a physical page, fills the PTE, and retries.
- Add copy-on-write (COW) pages for fork() support later.

**Acceptance criteria:**

- A kernel test allocates, frees, and re-allocates physical pages with no leaks.
- A user test can map and touch a new virtual page; the fault handler maps it on demand.
- `cargo test` still passes and `run.ps1` still prints the boot message.

### Milestone 1.2 — User-Mode Processes and System Calls

**Goal:** Introduce the process abstraction, user mode, and a proper syscall interface.

- Add user/supervisor privilege levels to Mik-64 and PTE flags (`PTE_U`).
- Add a system call table beyond `0` and `1` (e.g. `exit`, `fork`, `exec`, `sbrk`, `read`, `write`).
- Implement a simple `fork()` using COW pages.
- Implement `exec()` by replacing the process page table and loading a small user binary.
- Add a process table and PIDs.

**Acceptance criteria:**

- A hand-assembled user program can call `print_char` and `exit`.
- `fork` creates a child process that continues from the same PC.
- `exec` replaces the address space and starts at a new entry point.
- An integration test verifies a parent/child output sequence.

### Milestone 1.3 — Interrupts, Timer, and Preemptive Scheduling

**Status:** In progress — timer and `INT`/`IRET` foundation complete; round-robin
scheduler pending.

**Goal:** Replace the cooperative `TRAP` model with true interrupts and a preemptive scheduler.

- Add a programmable timer device (e.g. APIC-like) that raises an interrupt after a number of steps.
- Add an interrupt controller and `INT` / `IRET` instructions.
- Implement round-robin scheduling in the kernel.
- Save and restore process context (registers, page table, `pc`).

**Acceptance criteria:**

- Two user processes alternate output under the timer.
- A process can yield with a `sys_yield` syscall.
- The scheduler correctly saves and restores `PTBR`/`PMODE` on context switch.

### Milestone 1.4 — Tiny Mik-64 User Programs and Assembler

**Status:** In progress — text assembler works and produces runnable binaries; `exec` and pseudo file system pending.

**Goal:** Stop hand-assembling and build the smallest possible user-space build chain.

- Write a tiny Mik-64 assembler in Rust (text to flat binary).
- Define an ELF-like or flat binary user program format.
- Write a few small user programs in this assembly (e.g. `init`, `shell`, `cat`).
- Add a `READ` syscall and a simple in-memory pseudo file system or pipe for I/O.

**Acceptance criteria:**

- `mik-asm <foo.s> <foo.bin>` works and the output runs under `mik-emu`.
- The kernel can `exec` a user program built from the assembler.
- A user program can print a string and exit.

### Checkpoint: Mik-64 is a Miniature OS

- Multi-process scheduling works.
- Syscalls, page tables, demand paging, and user mode are exercised.
- The kernel is no longer hand-assembled byte-by-byte; the assembler produces it or user programs.
- All tests pass.

## Phase 2: Transition to x86-64 (QEMU/Bochs)

This phase is the educational bridge from the clean Mik-64 world to the real, quirky x86-64 world.

### Milestone 2.1 — x86-64 Bootloader and Long Mode

**Status:** In progress — minimal long-mode boot via the PVH direct-boot ABI works
(GDT, initial page tables, serial output). A custom boot sector / stage1 loader
is still pending.

**Goal:** Boot a real x86-64 kernel image under QEMU without any borrowed UEFI/GRUB code.

- Write a custom boot sector / stage1 loader that reads the kernel from a disk image or is embedded in a multiboot image.
- Switch from 16-bit real mode to 32-bit protected mode, then to 64-bit long mode.
- Set up a minimal GDT and IDT.
- Establish an initial page table for long mode.
- Print a character over serial (QEMU `-serial stdio`) as the first sign of life.

**Acceptance criteria:**

- `cargo run -p mik-os-x86 -- qemu` starts QEMU and prints a boot banner.
- The kernel is loaded above `0x400000` and begins executing in long mode.
- No hand-wavy BIOS calls remain in the boot path; the transition is fully self-contained.

### Milestone 2.2 — x86-64 Paging and Memory Management

**Goal:** Re-implement the Mik-64 memory concepts on real x86-64 page tables.

- Parse the memory map provided by the bootloader or BIOS/UEFI.
- Build a physical page allocator from available RAM.
- Implement the x86-64 version of the kernel identity mapping.
- Add `CR3` page table base switching for the first process.

**Acceptance criteria:**

- The kernel runs with 4-level paging enabled.
- A simple physical allocator can allocate and free 4 KiB frames.
- A user program can be mapped into a separate address space.

### Milestone 2.3 — x86-64 Interrupts, Syscalls, and Scheduling

**Goal:** Port the Mik-64 process model to x86-64.

- Set up the IDT for hardware exceptions and a timer (PIT/HPET/LAPIC).
- Use `syscall`/`sysret` or `int 0x80` for system calls.
- Implement context switch (save/restore `rsp`, `rflags`, `cs`, `ss`, page table).
- Port the round-robin scheduler from Mik-64.

**Acceptance criteria:**

- Timer interrupt fires and the scheduler switches processes.
- A user program can `sys_write` to the QEMU serial port and `sys_exit`.
- Two user programs run concurrently and interleave output.

### Milestone 2.4 — Demand Paging and Fork on x86-64

**Goal:** Bring over the richer memory features from Mik-64.

- Demand page faults allocate a physical frame and map it.
- `fork()` copies the page table with COW mappings.
- `exec()` replaces the address space and loads a new user program.

**Acceptance criteria:**

- A user program can `sbrk` and then access newly mapped memory.
- `fork` and `exec` integration tests pass.
- All prior `cargo test` equivalents still pass.

### Checkpoint: x86-64 Kernel Reproduces Mik-64 Behavior

- The x86-64 kernel can boot, schedule, handle syscalls, manage page tables, and run small user programs.
- The learning loop is closed: every concept proven in Mik-64 now works on real hardware (emulated).

## Phase 3: Real OS Features (Optional / Future)

Once the x86-64 kernel is solid, these features can be added in any order. They are deliberately left for later because each is a large topic on its own.

- **File system:** a simple in-memory or disk-backed file system (e.g. a minimal Mik-FS).
- **Console shell:** a tiny user-space shell that can run built-in commands.
- **Networking:** a very basic network stack over a virtual NIC.
- **Real device drivers:** keyboard, VGA text mode, PCI scanning.
- **Multi-core/SMP:** bootstrap additional CPUs.

## Non-Goals

These are deliberately out of scope to keep the project focused on understanding the core:

- Matching the full x86-64 instruction set or boot protocol in the Mik-64 phase.
- Production-grade performance, security hardening, or compatibility.
- A real GUI before the console and shell are solid.
- Real hardware boot in the first 6 months (QEMU/Bochs is the target).

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| The x86-64 long-mode boot dance is fiddly | High | Build it incrementally in QEMU; verify each mode transition with serial output before the next. |
| Real page tables and the TLB behave differently than Mik-64's | High | Write small focused tests in the x86-64 kernel before enabling user mode. |
| Multi-process context switch bugs are subtle | High | Keep the first scheduler non-preemptive until context save/restore is solid. |
| The hand-assembly bottleneck becomes painful | Medium | Prioritize the tiny assembler as soon as user programs are needed. |
| Scope creep into file systems / drivers | Medium | Treat Phase 3 as strictly optional until x86-64 scheduling and syscalls are complete. |

## How to Use This Roadmap

- Each milestone is a candidate for a focused implementation sprint.
- Before starting a milestone, write a spec and a detailed plan in `tasks/plan.md`.
- Update this roadmap when a milestone is complete or the long-term goal changes.
- The [Architecture](docs/architecture.md) and [Spec](docs/specs/mik-64.md) documents describe the current implementation and should be kept in sync with the roadmap.
