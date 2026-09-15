# ADR-009: Resumable Page-Fault Handler and Lazy `sbrk`

## Status

Accepted

## Date

2026-10-02

## Context

Milestone 2.4 requires demand paging: "Demand page faults allocate a physical
frame and map it" plus a `sbrk` userspace can use. Two sub-decisions:

1. **The fault path.** `#PF` (vector 14) is the first exception we handle
   that must *return* — the M2.1 stubs print-and-halt. Unlike `int`/IRQs,
   `#PF` pushes a CPU error code between the iret frame and the stub's saved
   registers, so the shared `IrqFrame` shape has to be restored by hand.
2. **What `sbrk` does.** Two models: eagerly map pages in the syscall, or
   move a boundary and let the fault handler map pages on first touch.

## Decision

**A dedicated `isr_pf` stub normalizes the stack into the common `IrqFrame`.**
`SAVE_REGS` runs first (preserving the real GPRs — clobbering `rsi` with the
error code beforehand was a real bug: the retry inherited the code as a
pointer), then the code is read at `[rsp+120]` into `rsi` as the handler's
second argument, and the five iret words are slid down over the code slot.
`pf_handler(frame, err)` returns a frame like every other kernel entry —
retrying the faulting instruction is just returning the same frame.

**`sbrk` is lazy.** It only advances `procs[cur].brk` (base `0x40002000`,
cap +64 KiB — a deliberate educational bound) and returns the old value. The
fault handler demand-maps: a not-present fault (`err` bit0 clear) with `cr2`
in `[USER_DATA_VA, brk)` allocates a zeroed frame, `map_4k`s it `P|W|U` into
the *current* process's table, `invlpg`s the page (CPUs may cache
not-present translations), and retries. Every other fault still prints
`EX0E` (now with `cr2`/`err`/`rip`/`rsi`/`rax` for debugging) and halts.

## Alternatives Considered

- **Eager `sbrk`**: mapping pages inside the syscall is simpler but doesn't
  exercise the fault path — the milestone exists to teach demand paging.
  Lazy `sbrk` is also what real kernels do (brk/mmap reserve VMA regions;
  faults populate them).
- **Extend the generic stub macro** to drop error codes for all
  error-code-pushing vectors (8, 10–14, 17, 30): cleaner symmetry, but those
  vectors still print-and-halt — only `#PF` needs the resumable shape today.
  Deferred; when a second returning exception arrives the slide can be
  generalized.
- **Region table instead of `brk`**: a VMA list would handle holes and
  permissions, but for a single growable heap a scalar boundary is the whole
  mechanism.

## Consequences

- Demand paging is ~15 lines of Rust: the fault classification is three
  comparisons (present?, in demand window?, write-on-COW-page?).
- `brk` lives in the process struct — `fork` inherits it, `exec` resets it.
- The `isr_pf` slide is the one piece of x86-specific bookkeeping the
  uniform-frame design can't hide; it is commented heavily because getting
  it wrong shifts every field of the frame.
- Limitation, marked in code: only the heap window demand-maps; a user fault
  elsewhere (bad pointer, stack overflow past the one-page stack) still
  kills the whole machine — process-kill-on-fault is the natural follow-up.
