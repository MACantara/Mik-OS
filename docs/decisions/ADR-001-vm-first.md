# ADR-001: Build a Custom Mik-64 VM Before Porting to x86-64

## Status

Accepted

## Date

2026-08-19

## Context

Mik OS is a personal learning project. The primary success criterion is
"I deeply understand OS internals", not shipping a production OS. The biggest
risk identified early was hardware support: real x86-64 has complex boot,
undocumented or poorly documented corners, and a large instruction set.

Three broad approaches were considered:

1. **Target x86-64 directly from day one.** Write a Rust `no_std` kernel, a
   custom target/linker, and run under QEMU.
2. **Design a simplified but realistic x86-64-like VM, then build the OS on it.**
3. **Use an existing emulator or architecture** such as RISC-V, and write the OS
   for that.

## Decision

Choose option 2. Define the **Mik-64** virtual machine, write a Rust emulator for
it, and build Mik OS there first. Once the core concepts (boot, memory
management, system calls, interrupts) are solid, port the design to real x86-64
under QEMU or Bochs.

## Alternatives Considered

### Option 1: x86-64 First

- **Pros:** Produces a real OS on real hardware; directly matches the long-term
  goal.
- **Cons:** The x86-64 boot and instruction set are complex; the learning curve
  is steep; many unrelated details can obscure the OS concepts. Real hardware
  quirks multiply the difficulty.
- **Rejected:** Too much complexity before the core ideas are clear.

### Option 3: Existing Architecture

- **Pros:** Can use an existing Rust target (e.g. RISC-V), write the kernel in
  Rust, and use existing toolchains.
- **Cons:** Still requires learning an architecture other than the target one.
  The chosen architecture might not be the one we eventually port to, so the
  mental model may not transfer cleanly.
- **Rejected:** A custom VM keeps the ISA as close to our learning needs as
  possible and lets us add only the features the OS requires.

## Consequences

- We must design and maintain a machine specification (`docs/specs/mik-64.md`)
  and a Rust emulator (`mik-emu`).
- The kernel cannot be compiled from Rust in the MVP because Rust has no
  Mik-64 target; it is hand-assembled (see ADR-002).
- We control the boot protocol, memory map, and instruction set, which keeps
  the OS code small and focused.
- The later x86-64 port will require a second implementation, but the OS
  concepts and structure will already be proven.
