# Mik OS

Mik OS is a from-scratch operating system built as a learning project. The first
phase runs on a custom 64-bit virtual machine called **Mik-64**, implemented by a
Rust emulator. The long-term goal is a real x86-64 port under QEMU or Bochs.

## Quick Start

Requires [Rust](https://rustup.rs/) and PowerShell (for `run.ps1`).

```powershell
# Build and run Mik OS in the Mik-64 emulator
cd C:\Projects\Mik-OS
.\run.ps1
```

Expected output:

```
!?Mik OS
```

The `!` is printed from a freshly allocated page; the `?` is printed through a
`print_char` system call; `Mik OS` is printed by direct serial I/O.

## Commands

| Command | Description |
|---------|-------------|
| `cargo build` | Build the emulator and kernel builder |
| `cargo test` | Run all emulator and kernel integration tests |
| `.\run.ps1` | Build and boot Mik OS in one step |
| `cargo run -p mik-os -- <path>` | Write the kernel flat binary to `<path>` |
| `cargo run -p mik-emu -- <path>` | Run a flat binary under the Mik-64 emulator |

## Architecture

```
┌─────────────────────────────────────┐
│  Mik OS kernel (hand-assembled)     │
│  - bump page allocator              │
│  - TRAP/ERET syscall handler        │
└─────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────┐
│  mik-emu (Rust)                     │
│  - interpreter for Mik-64 ISA       │
│  - flat memory + MMIO serial        │
│  - flat binary loader               │
└─────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────┐
│  Host (x86-64 Windows for now)      │
└─────────────────────────────────────┘
```

The full architecture and design rationale are in [`docs/architecture.md`](docs/architecture.md).

## Project Layout

```
docs/
  ideas/
    mik-os-vm-first.md     # Why and how we chose the VM-first approach
  specs/
    mik-64.md              # Complete Mik-64 machine specification
  architecture.md          # System architecture overview
  decisions/
    ADR-001-vm-first.md    # Why we built a custom VM first
    ADR-002-flat-memory-and-hand-assembly.md
mik-emu/
  src/lib.rs               # Emulator library
  src/main.rs              # CLI entry point
  tests/                   # Integration tests
mik-os/
  src/lib.rs               # Hand-assembled Mik-64 kernel
  src/main.rs              # Binary builder
  tests/kernel_boot.rs     # Kernel integration test
run.ps1                    # One-command build and run
tasks/
  plan.md                  # Implementation plan
  todo.md                  # Progress checklist
```

## Status

MVP complete. The emulator boots a hand-assembled Mik-64 kernel that:

- allocates physical memory with a bump allocator,
- sets up a trap vector at `0x2000`,
- handles `print_char` (syscall 1) and `halt` (syscall 0),
- prints to memory-mapped serial at `0x1000`.

## Next Steps

See [`tasks/plan.md`](tasks/plan.md). The most likely future directions are:

1. **Paging / virtual memory** for the Mik-64 emulator.
2. **Real x86-64 port** of Mik OS under QEMU, written in Rust for the actual
   target.
3. A tiny Mik-64 assembler so the kernel can be written in assembly rather than
   `mik_emu::encode` calls.

## License

This is a personal learning project. No license has been chosen yet.
