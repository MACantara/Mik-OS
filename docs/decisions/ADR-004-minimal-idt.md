# ADR-004: Minimal IDT with Print-and-Halt Exception Stubs

## Status

Accepted

## Date

2026-09-15

## Context

Milestone 2.1 requires an Interrupt Descriptor Table so the x86-64 kernel can
respond to CPU exceptions. The question was how complete that first IDT
should be: a minimal exception-only table, or an immediate jump to a full
setup with remapped PICs/APIC, IRQ handlers, and `iretq`-capable frames.

Timer/IRQ plumbing belongs to a later milestone (the scheduler port, M2.3);
only exceptions are needed now.

## Decision

Install a minimal IDT covering the 32 CPU exception vectors.

- `isr.S` defines 32 stubs via an assembler macro. Each pushes its vector
  number and jumps to a common handler that prints `EX` plus the vector as two
  hex digits on COM1, then `cli`/`hlt`s forever.
- `idt.rs` builds 32 interrupt-gate entries (`flags = 0x8E`, selector = the
  existing `gdt64` code segment) from an `isr_table` exported by the asm file,
  then `lidt`s it during `kmain`.
- `kmain` ends by executing `int3`, so every boot demonstrates vector 3 being
  delivered through the IDT (`EX03` on serial).

## Alternatives Considered

### Full IDT + IRQ Handlers Now

- **Pros:** Would already support the timer and keyboard when the scheduler
  port lands.
- **Cons:** Pulls PIC remapping or APIC setup, interrupt-safe frame handling,
  and `iretq` into a milestone that only asked for the IDT itself.
- **Rejected:** Deferred to M2.3 with the scheduler; the stub/table structure
  here is the scaffolding it will extend.

### Distinguish Error-Code Vectors in the Stubs

- **Pros:** Handlers could return (`iretq`) or inspect error codes.
- **Cons:** More stub variants and more frame bookkeeping for handlers that
  never return anyway.
- **Rejected:** The handlers are panic-style and never return, so a single
  uniform stub suffices; pushing only the vector keeps `isr_common` trivial.

## Consequences

- Any CPU exception now produces an identifiable `EXnn` line on serial instead
  of a silent triple fault — a real debugging win for everything after M2.1.
- The table is data-built in Rust from an asm-exported symbol table, so adding
  vectors (IRQs at 32+, a syscall gate) is an edit to `isr_table` and the IDT
  loop bound, not a redesign.
- Limitation, by design: there is no way to resume after an exception, and
  interrupt gates mean IF is cleared on entry — fine for panic handlers,
  wrong for the future timer tick, which will need a returning stub.
