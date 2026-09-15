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
- [x] Changes are committed, reviewed, and merged
- [x] ROADMAP.md is updated to show Milestone 2.1 in progress

# Mik OS Phase 1 Closeout Todo

## Checkpoint: Mik-64 is a Miniature OS

- [x] Task 1: Emulator `CSR_EPC` read + interrupt masking in handlers
- [x] Task 2: Process table, round-robin scheduler, context save/restore
- [x] Task 3: `fork` / `exec` / `yield` / `exit` syscalls
- [x] Task 4: Demand paging in the user region (`0x800000`..`0xA00000`)
- [x] Task 5: `init` / `prog1` user programs assembled by `mik-asm`
- [x] `cargo test` passes (`mik-os/tests/os.rs` runs two processes to halt)
- [x] ROADMAP.md is updated to show Phase 1 complete

Deferred: COW `fork`, pseudo file system / `READ` syscall.

# Mik OS x86-64 Milestone 2.1 Closeout Todo

## Checkpoint: x86-64 Boots from a Real Disk Image

- [x] Task 1: `.boot16` boot sector — INT 13h image load, A20, protected mode
- [x] Task 2: `.stage2` loader — copy flat kernel to `0x400000`, zero `.bss`,
      enter `_start` under the PVH-equivalent contract
- [x] Task 3: ELF section extraction + 64 KiB disk-image builder in
      `mik-os-x86` (`KEEP()` boot sections in `link.x`)
- [x] Task 4: Minimal IDT — 32 exception stubs, `EXnn` print-and-halt, `int3`
      demo in `kmain`
- [x] `cargo test -p mik-os-x86` passes (`disk_image.rs`,
      `boots_in_qemu.rs`)
- [x] QEMU BIOS boot prints `Mik-64 -> x86-64 long mode` + `EX03`; PVH path
      still works
- [x] ADR-003 (BIOS disk boot), ADR-004 (minimal IDT), concepts docs, ROADMAP
      updated

Deferred: resumable interrupt stubs, PIC/APIC IRQ plumbing (M2.3), memory-map
parsing (M2.2).