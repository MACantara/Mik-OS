# ADR-011: Non-Blocking Serial Input and a Ring-3 Shell

## Status

Accepted

## Date

2026-10-04

## Context

Phase 3 is a menu of optional real-OS features (filesystem, shell,
networking, drivers, SMP). The bare-minimum coherent slice is the console
shell — the first feature a user can actually interact with. The
sub-decisions:

1. How a ring-3 program receives keyboard/serial input at all: there is no
   keyboard driver, so the only plausible channel is the existing COM1
   UART, which QEMU exposes bidirectionally on `-serial stdio`.
2. Blocking vs. non-blocking read semantics: a blocking `read` needs a
   waiting state and a wake path (IRQ4 or timer recheck); a polled read
   needs none of that.
3. Whether the byte crosses the boundary via a user pointer or a register:
   a pointer needs address validation and fault handling; a register
   return is the ABI we already have.
4. Where the shell lives: kernel subroutine vs. a normal user process.

## Decision

**`sys_read` (syscall 7) returns one byte in `rax`, or `u64::MAX` when
empty.** `serial::read_byte` polls COM1's line status register
(`inb(0x3FD) & 1`) and reads the receive register only when data is
ready. No pointer crosses the boundary, so no user-address validation is
needed — the syscall is physically incapable of touching kernel memory.

**The shell polls and yields.** `prog_sh` in `user.S` prints `mik> `,
calls `sys_read`, and on empty input calls `sys_yield` instead of
spinning — the 100 Hz timer gives it the next slice anyway, so a busy
spin would only waste its own quantum, never stall the machine. Three
builtins: `v` prints a version banner, `q` exits via `sys_exit`, any
other byte is echoed. There is no line editing, no command history, no
newline semantics — input is a byte stream, not a terminal line.

**The shell is a third spawned process, not kernel code.** It is copied
into a user address space like every other program and scheduled
round-robin (`NPROC` 3→4 for A, B, shell; fork needs one free slot).
This reuses the entire M2.3/M2.4 pipeline unchanged — the shell's only
novelty is syscall 7.

## Alternatives Considered

- **IRQ4-driven input with a blocking `read`**: the "real" design —
  unmask UART RX in the PIC, buffer bytes on interrupt, sleep the reader
  until data arrives. Rejected for the minimal slice: it adds an ISR, a
  ring buffer, a `WAITING` process state, and a wake path, none of which
  the shell needs while the timer already reschedules it every 10 ms.
  Marked upgrade path.
- **Keyboard input (IRQ1) + VGA**: requires a scancode table, a line
  discipline, and a video writer — three subsystems for the same
  pedagogical result serial gives for ~10 lines.
- **`read` into a user buffer** (`read(fd, buf, len)`-shaped): needs
  validating `buf` against the private user range and surviving a
  mid-copy fault — machinery a one-byte register return avoids entirely.
- **Kernel-resident shell** (call a Rust function from `kmain`):
  trivially easier, but skips the point — the pedagogical payload is a
  *user-space* program doing I/O through the syscall gate.

## Consequences

- Serial output gains `mik> ` interleaved with the M2.4 demo sequence;
  the test filters to marker characters, and QEMU's piped stdin drives
  `v`/`q` to exercise the input path in CI.
- Non-blocking read is honest but primitive: two bytes arriving in one
  10 ms slice are handled fine (each read consumes one byte), and bytes
  typed while QEMU has no reader are buffered by the UART's FIFO, not
  lost.
- `q` marks the shell's slot `DEAD` permanently — there is no `respawn`
  and no way back to the prompt, matching the roadmap's educational
  scope.
- The syscall table is now 1=write, 2=exit, 3=yield, 4=fork, 5=exec,
  6=sbrk, 7=read — the same `rax`-number ABI, return in `rax`.
