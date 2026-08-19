# Mik OS — Agent Notes

## Project Goal

Build a from-scratch operating system named Mik OS. The first phase uses a
custom virtual machine called Mik-64 and a Rust emulator to learn OS internals
without first fighting x86-64 hardware. The long-term target is a real x86-64
port under QEMU or Bochs.

See [`README.md`](README.md) for the public project overview and quick start.

## Stack

- **Host language:** Rust
- **Emulator:** `mik-emu` crate in the workspace
- **Kernel language for the MVP:** Hand-assembled using `mik_emu::encode` in the
  `mik-os` crate. The real Rust Mik OS will come with the x86-64 port.

## Build and Test Commands

```bash
# Build everything
cargo build

# Run tests
cargo test

# Build and boot Mik OS in one command (PowerShell)
.\run.ps1

# Manual steps
cargo run -p mik-os -- target/mik-64-kernel.bin
cargo run -p mik-emu -- target/mik-64-kernel.bin
```

## Project Layout

```
docs/
  ideas/
    mik-os-vm-first.md      # Why and how we chose the VM-first approach
  specs/
    mik-64.md               # Complete Mik-64 machine specification
  architecture.md           # System architecture overview
  decisions/
    ADR-001-vm-first.md     # Why we built a custom VM first
    ADR-002-flat-memory-and-hand-assembly.md
README.md                   # Public project overview
mik-emu/
  src/lib.rs                # Emulator library
  src/main.rs               # CLI wrapper
  tests/                    # Integration tests
mik-os/
  src/lib.rs                # Hand-assembled Mik-64 kernel
  src/main.rs               # Binary builder
  tests/kernel_boot.rs      # Kernel integration test
run.ps1                     # One-command build/run
tasks/
  plan.md                   # Implementation plan with acceptance criteria
  todo.md                   # Progress checklist
```

## Important Decisions

- Flat physical memory model in the MVP; paging is added later.
- Mik-64 uses fixed 64-bit instruction words, a simple load/store architecture,
  and memory-mapped serial I/O.
- Initial boot: flat binary loaded at `0x400000`, `x15` (SP) set to `0x8000000`,
  PC set to `0x400000`.
- The trap vector lives at `0x2000`.
- The bump allocator keeps its `next_page` counter at `0x700000`.

See [`docs/decisions/`](docs/decisions/) for full ADRs.
