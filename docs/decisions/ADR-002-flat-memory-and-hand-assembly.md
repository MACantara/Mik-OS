# ADR-002: Flat Physical Memory and a Hand-Assembled Mik-64 Kernel

## Status

Accepted

## Date

2026-08-19

## Context

With the Mik-64 VM chosen (ADR-001), two further decisions were needed for the
MVP:

1. What memory model should the first version use?
2. How will the Mik-64 kernel be produced, given that Mik-64 is a custom ISA?

For (1), the options were a simple flat physical model or a full paging model
with page tables, TLBs, and page faults. For (2), the options were to hand
assemble the kernel, switch the VM to a Rust-supported architecture such as
RISC-V, or build a Rust compiler backend for Mik-64.

## Decision

1. Use a **flat physical memory model** in the MVP. Every address is a physical
   byte index. Page tables, virtual memory, and protection are deferred.

2. **Hand-assemble the Mik-64 kernel** using `mik_emu::encode` in the `mik-os`
   crate. The real Rust Mik OS will be written later for the x86-64 port.

## Alternatives Considered

### Paging in the MVP

- **Pros:** More realistic; enables per-process address spaces and memory
  protection.
- **Cons:** Adds page tables, address translation, page faults, and permission
  handling to the emulator and the kernel. This is a major complexity jump
  before the basics of boot, I/O, and allocation are proven.
- **Rejected:** Flat memory is sufficient for learning the first OS concepts and
  can be upgraded later once the kernel is stable.

### Switch the VM to a Rust-Supported Architecture

- **Pros:** The kernel could be written in Rust immediately using an existing
  target such as `riscv64gc-unknown-none-elf`.
- **Cons:** We would have to abandon or rewrite the custom Mik-64 design. The
  architecture might be more complex than needed, and the eventual x86-64 port
  would still be a second effort.
- **Rejected:** Keeping the custom VM is a deliberate learning choice.

### Build a Rust Compiler Backend for Mik-64

- **Pros:** The kernel could be written in Rust for Mik-64 directly.
- **Cons:** Writing a LLVM/Rust backend for a custom ISA is far beyond the
  scope of a learning project.
- **Rejected:** Not feasible for an MVP.

## Consequences

- The kernel is a `no_std`-style host program that emits a flat binary; the
  real `no_std` Rust kernel comes later on x86-64.
- The emulator and kernel are small: the MVP fits in a few hundred lines of
  Rust and hand-assembled instructions.
- The bump allocator is simple and deterministic because addresses are direct
  physical indices.
- When we later add paging, the allocator will need to manage physical frames
  and the CPU will need a page-table walker. This is a known, deferred scope.
