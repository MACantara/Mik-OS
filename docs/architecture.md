# Mik OS Architecture

This document describes how the Mik-64 virtual machine, the Rust emulator, and
the hand-assembled Mik-64 kernel fit together.

## Goals

- Learn operating-system internals by building a complete, end-to-end system.
- Start with a controlled, simple environment (a custom VM) before moving to real
  x86-64 hardware.
- Keep the instruction set and emulator small enough to fit in one head.

## Non-goals

- Match the full x86-64 instruction set or boot protocol.
- Support real hardware in the MVP.
- Write a production-grade compiler or full user-space shell.

## Layers

### 1. Mik-64 Machine

Mik-64 is a 64-bit load/store machine with:

- 16 general-purpose 64-bit registers (`x0` is hard-wired to zero).
- Fixed 64-bit instruction words.
- 128 MiB of flat physical memory.
- Memory-mapped serial I/O at `0x1000`.
- A trap vector at `0x2000`.
- A flat binary boot protocol: the binary is loaded at `0x400000` and the PC is
  set to `0x400000`.

See [`specs/mik-64.md`](specs/mik-64.md) for the full specification, including
all opcodes and semantics.

### 2. mik-emu

`mik-emu` is a straightforward Rust interpreter. For each step it:

1. Fetches the 64-bit instruction at `pc`.
2. Decodes the opcode, register fields, and 44-bit immediate.
3. Sign-extends the immediate.
4. Dispatches to the appropriate handler.

The emulator is intentionally an interpreter, not a JIT. This makes the
instruction-by-instruction behavior transparent and easy to debug.

Key files:

- `mik-emu/src/lib.rs` — `Machine`, instruction decode/execute, `run()` entry.
- `mik-emu/src/main.rs` — CLI that loads a flat binary and runs it.
- `mik-emu/tests/*.rs` — integration tests for instructions and the full
  kernel.

### 3. mik-os Kernel

The MVP kernel is hand-assembled using `mik_emu::encode` because Mik-64 is a
custom ISA and there is no Rust target for it. The kernel demonstrates:

- **Boot and I/O:** prints characters through the serial MMIO port.
- **Physical memory management:** a bump allocator that tracks the next free
  4 KiB page at address `0x700000`.
- **Subroutines:** uses `JMPR` for returns from `alloc_page` and `free_page`.
- **System calls:** sets a trap vector at `0x2000`, then uses `TRAP`/`ERET` to
  enter and exit a syscall handler.
- **Paging:** builds a 4-level identity-mapped page table, sets `PTBR`, enables
  `PMODE`, and continues executing with virtual memory.

The kernel flat binary is produced by the `mik-os` crate and then loaded by
`mik-emu`.

## Memory Map

```
0x0000_0000 .. 0x0000_0FFF  : reserved zero page
0x0000_1000                 : serial data port
0x0000_1001                 : serial status port
0x0000_1002 .. 0x0000_1FFF  : reserved
0x0000_2000                 : trap vector (64-bit handler address)
0x0000_2008 .. 0x003F_FFFF  : reserved
0x0040_0000 .. 0x7FFF_FFFF  : general RAM (126 MiB)
0x7000_0000                 : bump-allocator next_page counter (kernel convention)
0x8000_0000                 : initial stack pointer (top of RAM)
```

The kernel is loaded at `0x400000`.

## Boot Flow

1. `mik-emu` creates a `Machine`, zeroes memory, and sets `pc = 0x400000`.
2. The kernel flat binary is copied into RAM starting at `0x400000`.
3. The emulator starts executing instructions.
4. The kernel initializes:
   - `next_page` at `0x700000` to `0x701000`.
   - The trap vector at `0x2000` to the syscall handler.
   - The page-fault vector at `0x2010` to a placeholder handler.
5. The kernel calls `alloc_page`, writes a character, prints it, saves the
   page, then allocates 7 consecutive pages for a PML4, PDPT, PD, and four PTs.
6. The kernel fills the page tables with an identity mapping for the low 8 MiB,
   writes `PTBR`, and enables `PMODE`.
7. With paging on, the kernel frees the demo page, invokes a `print_char`
   syscall, prints the rest of the boot message, and finally executes `TRAP 0`
   to halt.

## Syscall Flow

1. User code sets argument(s), e.g. `x2 = '?'`, then executes `TRAP 1`.
2. The emulator:
   - saves the return address in the internal `epc` register,
   - writes the syscall number to `x10`,
   - loads the handler address from `0x2000`,
   - sets `pc` to the handler.
3. The handler inspects `x10`:
   - `0` → `HALT`
   - `1` → print the low byte of `x2` to `0x1000`
   - anything else → `ERET`
4. The handler executes `ERET`, which sets `pc = epc`.
5. Execution continues with the instruction after `TRAP`.

## Build and Run Pipeline

```
mik-os/src/lib.rs  --encode-->  flat binary  --load-->  mik-emu  --stdout
```

`run.ps1` ties the steps together:

1. `cargo build`
2. `cargo run -p mik-os -- target/mik-64-kernel.bin`
3. `cargo run -p mik-emu -- target/mik-64-kernel.bin`

## Testing

Tests are at two levels:

- **Instruction tests** in `mik-emu/tests/` verify individual opcodes such as
  `JMPR` and `TRAP`/`ERET`.
- **Kernel integration test** in `mik-os/tests/kernel_boot.rs` verifies the full
  boot output: `!?Mik OS\n`.
- **Paging integration test** in `mik-os/tests/kernel_paging.rs` verifies the
  kernel enables `PMODE` and `PTBR` and still prints the boot message.

Run all tests with `cargo test`.

## Future Directions

- Task 12: add a real kernel page-fault handler that inspects `x10`/`x11`.
- Port Mik OS to real x86-64 under QEMU, written in Rust.
- Build a small Mik-64 assembler so the kernel can be written as text assembly.
