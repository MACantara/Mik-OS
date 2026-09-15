# Mik OS

Mik OS is a from-scratch operating system built as a learning project. It has
two implementations that share one design:

- **Mik-64** — a custom 64-bit virtual machine and Rust emulator, used as a
  safe sandbox to prove each OS concept.
- **x86-64** — a `#![no_std]` Rust kernel that boots a real BIOS disk image
  under QEMU and runs ring-3 processes on a timer-driven scheduler.

## Quick Start

Requires [Rust](https://rustup.rs/) and PowerShell (for `run.ps1`).

```powershell
# Phase 1: build and run Mik OS in the Mik-64 emulator
cd C:\Projects\Mik-OS
.\run.ps1
```

Expected output (interleaving may vary under the timer):

```
!ICPEQD
```

`!` is the kernel's boot demo; `I` is `init` starting; `C`/`P` are the forked
child and parent; `Q`/`D` are printed by `prog1` after the child `exec`s it
(the `Q` store demand-faults a heap page in first); `E` is the parent exiting.

```powershell
# Phase 2+: boot the real x86-64 kernel under QEMU
rustup target add x86_64-unknown-none   # one-time
cargo run -p mik-os-x86 -- qemu
```

This builds a 128 KiB BIOS disk image and boots it. After the boot report you
get a `mik> ` prompt on the serial console — type `v` for the version banner,
any other key to echo, `q` to exit the shell. `Ctrl-C` stops QEMU (the kernel
idles forever by design).

`mik-os-x86` finds QEMU via `$QEMU`, then `PATH`, then `C:\Program Files\qemu`.

## Commands

| Command | Description |
|---------|-------------|
| `cargo build` | Build the whole workspace |
| `cargo test` | Run all emulator, kernel, and QEMU integration tests |
| `.\run.ps1` | Build and boot the Mik-64 kernel in one step |
| `cargo run -p mik-os -- <path>` | Write the Mik-64 kernel flat binary to `<path>` |
| `cargo run -p mik-emu -- <path>` | Run a flat binary under the Mik-64 emulator |
| `cargo run -p mik-asm -- <in.s> <out.bin>` | Assemble a Mik-64 source file to a flat binary |
| `cargo run -p mik-os-x86 -- qemu` | Build the disk image and boot the BIOS path in QEMU |
| `cargo run -p mik-os-x86 -- pvh` | Boot the same kernel via QEMU's direct `-kernel` path |
| `cargo run -p mik-os-x86 -- image` | Build `target/mik-os-x86.img` without booting |

## Architecture

Two stacks, one design — Mik-64 proves each concept, then the x86-64 kernel
re-implements it on real hardware:

```
Mik-64 sandbox (Phase 1)              x86-64 port (Phase 2+)
┌──────────────────────────┐          ┌──────────────────────────┐
│ mik-os kernel + init,    │          │ Rust no_std kernel       │
│ prog1 (mik-asm)          │          │ ring-3 progs + shell     │
│ free-list, demand paging,│          │ E820 frames, 4-level     │
│ 2 procs, fork/exec, timer│          │ paging, COW fork, exec   │
└──────────────────────────┘          │ IDT/PIC/PIT, int 0x80    │
              │                       └──────────────────────────┘
              ▼                                 │
┌──────────────────────────┐                    ▼
│ mik-emu (interpreter)    │          ┌──────────────────────────┐
└──────────────────────────┘          │ BIOS boot16 -> stage2 -> │
              │                       │ long mode (QEMU)         │
              ▼                       └──────────────────────────┘
        Host (Windows)
```

The full architecture and design rationale are in
[`docs/architecture.md`](docs/architecture.md); per-milestone decisions are in
[`docs/decisions/`](docs/decisions/).

## Project Layout

```
ROADMAP.md                 # Long-term plan and milestones
docs/
  ideas/                   # Early direction documents
  specs/mik-64.md          # Complete Mik-64 machine specification
  concepts/                # Per-milestone explainers + why-they-matter docs
  decisions/               # ADR-001 .. ADR-011
  architecture.md          # System architecture overview
mik-emu/                   # Mik-64 emulator (library + CLI + tests)
mik-asm/                   # Text-to-binary Mik-64 assembler
mik-os/
  src/lib.rs               # Hand-assembled Mik-64 kernel
  user/                    # init.s, prog1.s — assembled at build time
  tests/                   # Kernel integration tests (boot, paging, OS demo)
mik-os-x86/
  src/                     # Runner: image builder + QEMU launcher
  kernel/src/              # no_std kernel: boot16/stage2, mem, sched, user.S
  tests/                   # build_kernel, disk_image, boots_in_qemu
run.ps1                    # One-command Mik-64 build and run
tasks/                     # Implementation plan and progress checklist
```

## Status

- **Phase 1 (Mik-64): complete.** Two user processes under a timer-driven
  round-robin scheduler with per-process page tables, demand paging, and
  `fork`/`exec`/`yield`/`exit`.
- **Phase 2 (x86-64): complete.** BIOS disk boot to long mode; E820-fed
  frame allocator and 1 GiB identity map; ring-3 processes with GDT/TSS,
  a 100 Hz PIT tick, `int 0x80` syscalls, and frame-return context
  switching; resumable page faults, lazy `sbrk`, COW `fork`, and `exec`.
- **Phase 3: begun.** M3.1 added a serial console shell — `sys_read`
  (non-blocking COM1 poll) plus a ring-3 shell with `v`/`q`/echo builtins.

## Next Steps

See [`ROADMAP.md`](ROADMAP.md). Remaining Phase 3 candidates (all optional):
a simple filesystem, networking over a virtual NIC, keyboard/VGA drivers,
and SMP.

## License

MIT
