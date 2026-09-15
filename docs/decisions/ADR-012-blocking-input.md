# ADR-012: Interrupt-Driven Input and Restartable Blocking `read`

## Status

Accepted

## Date

2026-10-05

## Context

Milestone 3.2 turns `sys_read` from a non-blocking poll into a real blocking
read. The pieces that had to be decided:

1. Where input bytes wait between the device interrupt and the reader's
   `sys_read` call.
2. How a process blocks: what "waiting for input" means to a scheduler
   that only knows `READY`/`DEAD`.
3. How the byte gets into the process's `rax` when the process asked
   *before* the byte existed.
4. What happens when every runnable process is blocked.

## Decision

**One shared 64-byte input ring buffer.** `input.rs` is a lock-free-by-
gate ring: producers (keyboard IRQ1, UART IRQ4) run inside interrupt gates
and the consumer (`sys_read`) inside the `int 0x80` gate — all with IF=0,
so no push/pop can ever interleave. Overflow drops bytes (64 bytes of
untyped input is beyond this shell's needs).

**`WAITING` is a real process state.** `sys_read` on an empty buffer marks
the process `WAITING`, rewinds `frame.rip` by 2 — `int 0x80` is `CD 80`,
so resume re-executes the syscall — and `schedule()`s away. An ISR that
pushes a byte calls `wake_on_input()`, flipping every `WAITING` process
back to `READY`. The woken process's next dispatch re-traps into
`sys_read`, pops the byte, and returns it in `rax` — the classic
restartable-syscall pattern (Linux's `ERESTARTSYS`), implemented in three
lines because the frame is already the whole context.

**An idle loop, not a deadlock.** If `schedule()` finds nothing `READY` but
some process `WAITING`, it parks in `sti; hlt` under an `IN_IDLE` gate that
makes the timer handler drop ticks — a tick fired there would save an
idle-loop frame over the blocked process's real frame. The first input IRQ
marks the waiter `READY`, the spin sees it, and `schedule()` returns its
frame normally. (Unreachable while the never-sleeping demo process B lives,
but the path exists because correctness shouldn't depend on B's habits.)

**Interrupt fast path + polling safety net.** `uart_handler` drains RBR on
IRQ4, and `timer_handler` calls the same `drain_rx()` every tick — a byte
that arrived inside an IF=0 window, or whose edge-triggered IRQ was missed
or coalesced by an emulated UART, still reaches the buffer within 10 ms.
Empirically necessary: QEMU's stdio chardev on Windows delivers the first
byte of a burst and holds the rest host-side; the FIFO enable (`FCR`) plus
the tick-side drain keeps input flowing regardless.

## Alternatives Considered

- **Deliver the byte into the waiting frame directly** (ISR writes
  `frame.rax` and returns the waiter's frame — an early-return wake):
  removes the restart trick but creates two delivery paths (buffer vs.
  frame) and a "which waiter" question. The restart keeps `sys_read` the
  only consumer of the buffer — one path, no races.
- **Return `-1` and keep polling** (M3.1 design): simpler but burns slices
  and isn't an honest `read` — the milestone's point is interrupt-driven
  I/O.
- **Per-process wait queues**: overkill — one wait reason (input) exists, so
  a global `WAITING` state + wake-all is correct and ~10 lines.
- **Level-triggered PIC mode** for IRQ4: 8259 level mode needs careful EOI
  ordering to avoid spurious-IRQ storms; edge + the tick drain is more
  robust on emulators.

## Consequences

- `sys_read` never returns `-1`; it returns a byte or doesn't return until
  one exists. User code is simpler (no retry loop) but a blocked reader
  cannot be killed — no such mechanism exists anyway.
- The input path works identically from keyboard (IRQ1) and serial (IRQ4):
  both decode to bytes in the same buffer, which is exactly what a tty
  abstraction will want later.
- The shell gained line discipline in *user space* (echo, Enter submits,
  backspace erases) — the kernel still sees a byte stream. A kernel-side
  canonical-mode tty is a deferred upgrade.
- New deferred work: IRQ coalescing on real 16550 FIFO trigger levels,
  wait-queue abstraction when a second wait reason appears (disk I/O in
  M3.3 is the natural candidate), Ctrl-C-style signal delivery.
