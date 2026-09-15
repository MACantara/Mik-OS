# Mik OS Architecture

This document describes the two implementations of Mik OS and how they fit
together: the Mik-64 virtual machine, emulator, and hand-assembled kernel
(Phase 1), and the Rust `no_std` x86-64 kernel that ports the same design to
real hardware interfaces under QEMU (Phase 2+).

## Goals

- Learn operating-system internals by building a complete, end-to-end system.
- Start with a controlled, simple environment (a custom VM) before moving to
  real x86-64 hardware — each concept is proven on Mik-64 first, then ported.
- Keep the instruction set and emulator small enough to fit in one head.

## Non-goals

- Match the full x86-64 instruction set or boot protocol in the Mik-64 phase.
- Production-grade performance, security hardening, or compatibility.

## Layer 1: Mik-64 Machine

Mik-64 is a 64-bit load/store machine with:

- 16 general-purpose 64-bit registers (`x0` is hard-wired to zero).
- Fixed 64-bit instruction words.
- 128 MiB of flat physical memory.
- Memory-mapped serial I/O at `0x1000`.
- A trap vector at `0x2000`, a page-fault vector at `0x2010`, and a
  timer/interrupt vector at `0x2020`.
- Optional 4-level paging (`PTBR`, `PMODE`), user/supervisor mode (`SRET`,
  `PTE_U`), software interrupts (`INT`/`IRET`), and a programmable timer.

See [`specs/mik-64.md`](specs/mik-64.md) for the full specification, including
all opcodes and semantics.

## Layer 2: mik-emu

`mik-emu` is a straightforward Rust interpreter. For each step it fetches the
64-bit instruction at `pc`, decodes opcode/register/immediate fields, and
dispatches to a handler. It also models the timer (decrement-and-interrupt)
and enforces `PTE_U` when paging is enabled.

The emulator is intentionally an interpreter, not a JIT — the
instruction-by-instruction behavior is transparent and easy to debug.

Key files:

- `mik-emu/src/lib.rs` — `Machine`, instruction decode/execute, `run()` entry.
- `mik-emu/src/main.rs` — CLI that loads a flat binary and runs it.
- `mik-emu/tests/*.rs` — instruction-level tests.

## Layer 3: mik-os Kernel (Mik-64)

The Mik-64 kernel is hand-assembled: `mik-os/src/lib.rs` builds the binary
through `mik_emu::encode` and `mik-asm`, and `mik-os/user/{init,prog1}.s` are
assembled to flat binaries at kernel-build time. The kernel demonstrates:

- **Physical memory management:** a free-list allocator (`alloc_page` /
  `free_page`, head at `0x700008`) falling back to the bump counter
  `next_page` at `0x700000`.
- **Paging:** a 4-level identity-mapped kernel table plus per-process
  page-table chains that share it; `PTBR`/`PMODE` control translation.
- **Demand paging:** not-present faults in the user region
  `0x800000..0xA00000` allocate a frame, fill the PTE, and retry.
- **User mode and syscalls:** `SRET` enters ring-U; `TRAP`/`ERET` implement
  the syscall round trip (`halt`, `print_char`, `fork`, `exec`, `yield`,
  `exit`).
- **Processes and scheduling:** a two-slot process table at `0x700100`, each
  with its own page-table chain, saved registers, and kernel state; a
  programmable timer drives round-robin preemption through `INT`/`IRET`.
- **User programs:** `init` forks, the child `exec`s `prog1` (which
  demand-faults a heap page before printing), and the parent yields and
  exits.

The kernel flat binary is produced by the `mik-os` crate and loaded by
`mik-emu`.

## Layer 4: mik-os-x86 Kernel (x86-64)

`mik-os-x86/kernel` is a `#![no_std]`, `#![no_main]` Rust kernel for
`x86_64-unknown-none`, plus a small runner crate that builds the bootable
disk image and launches QEMU.

**Boot chain** (`boot16.S`, `stage2.S`, `boot.S`): a 512-byte BIOS boot
sector at `0x7C00` collects the E820 memory map to phys `0x5000`, loads
sectors 1–255 via INT 13h, enables A20, and enters protected mode; `stage2`
at `0x7E00` copies the flat kernel to `0x400000` and reproduces the PVH
handoff so `boot.S`'s long-mode transition is shared by both paths. The
runner's ELF parser packs these sections into a 128 KiB raw image — no
`objcopy` needed.

**Memory** (`e820.rs`, `mem.rs`): a free-list frame allocator over E820-usable
regions (excluding <1 MiB and the kernel image), a kernel-owned 1 GiB
identity map of 2 MiB pages, and `map_4k`/`build_user_table` for per-process
PML4s that share the kernel PD at PDPT[0] with private user pages at
`0x40000000`+ (ring-3 needs `PTE_U` at *every* walk level).

**Interrupts and scheduling** (`seg.rs`, `pic.rs`, `idt.rs`, `isr.S`,
`sched.rs`): GDT user segments + TSS with per-process kernel stacks; both
PICs remapped to vectors 32–47 with only IRQ0 unmasked; PIT at ~100 Hz;
a 256-entry IDT where the timer and `int 0x80` share one contract — the CPU
pushes the iret frame, the stub pushes 15 GPRs, and the Rust handler returns
whichever `IrqFrame` to resume (returning another process's frame *is* the
context switch, `CR3` included). `sched::start` drains the BIOS-latched
tick before the first dispatch.

**Syscalls** (`int 0x80`, `rax`=number, return in `rax`): 1 write,
2 exit, 3 yield, 4 fork, 5 exec, 6 sbrk, 7 read. Page faults are resumable:
`isr_pf` normalizes the CPU error code into the shared frame layout, and
`pf_handler` demand-maps `[0x40002000, brk)` and COW-copies private
write faults. `fork` shares leaf frames read-only on both sides (no
refcount — marked simplification); `exec` swaps in a fresh user table and
rewrites the live frame in place.

**Shell** (`user.S`, `serial.rs`): `prog_sh` runs as a third spawned
process — prints `mik> `, polls `sys_read` (non-blocking COM1 LSR poll
returning a byte or `-1`), yields while idle, and answers `v`/`q`/echo.

## Memory Maps

### Mik-64

```
0x0000_0000 .. 0x0000_0FFF  : reserved zero page
0x0000_1000                 : serial data port
0x0000_1001                 : serial status port
0x0000_2000                 : trap vector (64-bit handler address)
0x0000_2010                 : page-fault vector
0x0000_2020                 : timer / interrupt vector
0x0040_0000 .. 0x07FF_FFFF  : general RAM (124 MiB)
0x0070_0000                 : next_page bump counter + reserved metadata page
                              (allocator state, scratch, process table)
0x0080_0000 .. 0x00A0_0000  : user VA region (per-process PT4, demand-paged)
0x0800_0000                 : initial stack pointer (top of RAM)
```

The kernel is loaded at `0x400000`.

### x86-64 (physical)

```
0x0000_7C00                 : BIOS loads the boot sector here
0x0000_7E00 ..              : stage2 + kernel blob loaded by INT 13h
0x0000_5000                 : E820 map buffer (magic + count + entries)
0x0040_0000 ..              : flat kernel image, copied by stage2
0x0000_0000 .. 0x3FFF_FFFF  : 1 GiB identity map (2 MiB pages, kernel-owned)
0x4000_0000 .. 0x8000_0000  : private user region per address space
```

## Build and Run Pipeline

```
Mik-64:  mik-os/user/*.s --mik-asm--> flat user bins
         mik-os/src/lib.rs --encode--> kernel flat binary --load--> mik-emu

x86-64:  kernel ELF (.boot16/.stage2/.text...) --mik-os-x86 image-->
         128 KiB raw disk image --qemu -drive--> BIOS -> INT 13h ->
         stage2 -> long mode -> kmain
         (or: qemu -kernel <elf> for the PVH comparison path)
```

`run.ps1` ties the Mik-64 steps together; `cargo run -p mik-os-x86 -- qemu`
builds and boots the x86-64 path.

## Testing

- **Mik-64:** instruction tests in `mik-emu/tests/`; kernel integration tests
  in `mik-os/tests/` cover boot, paging, the page-fault handler, the free
  list, the timer, user mode, and the full two-process OS demo
  (`os.rs` asserts the `I`/`P`/`C`/`Q`/`D`/`E` ordering invariants).
- **x86-64:** `build_kernel` (compiles for `x86_64-unknown-none`),
  `disk_image` (128 KiB layout, `0xAA55`, patched kernel length), and
  `boots_in_qemu` (boots the real image, asserts the `ADcEDp` demand/COW/exec
  sequence, and drives the shell by writing `vq` into QEMU's stdin).

Run everything with `cargo test`; QEMU-dependent tests skip gracefully when
QEMU is not installed.

## Future Directions

- Remaining Phase 3 items (all optional): a simple filesystem, networking
  over a virtual NIC, keyboard/VGA drivers, SMP.
- Known simplifications to revisit: blocking `read` over UART IRQ4, line
  discipline for the shell, frame/table reclamation on `exec`/`exit`,
  refcounted COW, kill-on-fault instead of halt, a higher-half kernel.
