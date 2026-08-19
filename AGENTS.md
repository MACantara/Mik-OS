# Mik OS — Agent Notes

## Project Goal

Build a from-scratch operating system named Mik OS. The first phase uses a custom virtual machine called Mik-64 and a Rust emulator to learn OS internals without first fighting x86-64 hardware. The long-term target is a real x86-64 port under QEMU or Bochs.

## Stack

- **Host language:** Rust
- **Emulator:** `mik-emu` crate in the workspace
- **Kernel language for the MVP:** Hand-assembled using `mik_emu::encode` in the `mik-os` crate. The real Rust Mik OS will come with the x86-64 port.

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
    mik-os-vm-first.md   # Refined direction
  specs/
    mik-64.md            # Mik-64 machine specification
mik-emu/
  src/lib.rs             # Emulator library
  src/main.rs            # CLI wrapper
  tests/hello_mik.rs     # Integration test: "Hello, Mik!"
tasks/
  plan.md                # Implementation plan with acceptance criteria
  todo.md                # Checklist
```

## Important Decisions

- Flat physical memory model in the MVP; paging is added later.
- Mik-64 uses fixed 64-bit instruction words, a simple load/store architecture, and memory-mapped serial I/O.
- Initial boot: flat binary loaded at `0x400000`, `x15` (SP) set to `0x8000000`, PC set to `0x400000`.

## Open Decisions

See `tasks/plan.md` for the open question about how to write the Mik-64 kernel in the MVP, given that Mik-64 is a custom ISA.
