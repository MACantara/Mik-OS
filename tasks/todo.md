# Mik OS User Mode Todo

## Milestone 1.2: User-Mode Processes and System Calls

- [x] Task 1: Add emulator support for user mode (PTE_U, SRET, TRAP/ERET mode switch)
- [x] Task 2: Build kernel user-mode binary and integration test

## Checkpoint: User Mode Works End-to-End

- [x] `cargo test` passes
- [x] `run.ps1` still works
- [x] A user program can trap to the kernel and back
- [x] Changes are committed, reviewed, and merged

# Mik OS Timer and Interrupts Todo

## Milestone 1.3: Interrupts, Timer, and Preemptive Scheduling

- [x] Task 1: Add emulator timer and `INT`/`IRET` support
- [x] Task 2: Build `kernel_timer` binary and integration test

## Checkpoint: Timer and Interrupts Work End-to-End

- [x] `cargo test` passes
- [x] `run.ps1` still works
- [x] A user program can be interrupted and resume with `IRET`
- [x] Changes are committed, reviewed, and merged
- [x] `ROADMAP.md` is updated to show Milestone 1.3 in progress

# Mik OS Tiny Assembler Todo

## Milestone 1.4: Tiny Mik-64 User Programs and Assembler

- [x] Task 1: Create `mik-asm` workspace crate with text assembler
- [x] Task 2: Build a Hello-world user program and integration test

## Checkpoint: Assembler Works End-to-End

- [x] `cargo test` passes
- [x] `mik-asm <foo.s> <foo.bin>` produces a runnable binary
- [x] `mik-emu <foo.bin>` prints `Hello\n`
- [x] Changes are committed, reviewed, and merged
- [x] `ROADMAP.md` is updated to show Milestone 1.4 in progress


# Mik OS x86-64 Boot Todo

## Milestone 2.1: x86-64 Bootloader and Long Mode

- [x] Task 1: Install x86_64-unknown-none target and QEMU
- [x] Task 2: Create mik-os-x86 workspace crate with long-mode boot
- [x] Task 3: Build and run under QEMU, verify serial banner

## Checkpoint: x86-64 Boots into Long Mode

- [x] cargo build -p mik-os-x86 produces an ELF image
- [x] cargo run -p mik-os-x86 -- qemu starts QEMU and prints a banner
- [ ] Changes are committed, reviewed, and merged
- [ ] ROADMAP.md is updated to show Milestone 2.1 in progress