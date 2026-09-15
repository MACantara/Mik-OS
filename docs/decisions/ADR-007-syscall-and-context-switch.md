# ADR-007: `int 0x80` Syscalls and Frame-Return Context Switching

## Status

Accepted

## Date

2026-10-02

## Context

Milestone 2.3 requires a system-call mechanism (`syscall`/`sysret` or
`int 0x80`) and a context switch that saves/restores `rsp`, `rflags`, `cs`,
`ss`, and page-table state — the full process state needed to pause one ring-3
program and resume another.

Two design questions follow:

1. How does a user program cross into the kernel? `syscall`/`sysret` is the
   modern fast path: `syscall` jumps to an `MSR`-configured `rip` with a
   fixed `cs`/`ss`, saving the return address in `rcx` and `rflags` in `r11`.
   `int 0x80` is a software interrupt: it goes through an IDT gate, switches
   to `TSS.rsp0`, and pushes the complete iret frame (`ss`, `rsp`, `rflags`,
   `cs`, `rip`).
2. How does the kernel store and switch a process's context? Options range
   from a dedicated per-process `Context` struct switched by an explicit
   `switch` routine, to making every kernel entry produce the same stack
   frame and letting the handler pick which frame to resume.

## Decision

**`int 0x80` for syscalls, and a single `IrqFrame` contract for every kernel
entry.** Both the timer IRQ and `int 0x80` arrive the same way: the CPU pushes
the iret frame on the ring-0 stack taken from `TSS.rsp0`, then `SAVE_REGS`
pushes the 15 GPRs. The Rust handler receives a pointer to that frame and
returns a pointer to the frame that should run next. Returning the *same*
frame resumes the interrupted process; returning a *different* process's
saved frame is the entire context switch — `irq_tail` pops the registers and
`iretq` restores `rip`, `cs`, `rflags`, `rsp`, and `ss` in one instruction.

The first dispatch fabricates an `IrqFrame` on a fresh kernel stack
(`rip` = user code VA, `cs`/`ss` = RPL-3 selectors, `rflags` = `0x202`,
`rsp` = user stack top) and enters through `enter_user`, which is just
`irq_tail` reached by `mov rsp, rdi` — first entry is the same code path as
every later re-entry.

`int 0x80` is an interrupt gate (`0x8E` | DPL 3 = `0xEE`), so `IF` clears on
entry: a timer tick cannot preempt a syscall handler mid-schedule.

## Alternatives Considered

- **`syscall`/`sysret`**: faster and is what real x86-64 kernels use, but it
  does *not* save `rsp`, `ss`, or `rflags` in a frame — the kernel must save
  user `rsp` itself and reconstruct the rest. Two entry mechanisms (syscall
  vs. interrupt frame) means two context representations to keep in sync.
  `int 0x80` gives one frame shape for everything; `syscall` is the natural
  performance follow-up (needs `MSR` setup + a separate return path).
- **Dedicated `switch` routine** (cooperative kernel stacks, save callee-saved
  regs only): smaller per-switch cost, but splits process state across two
  places — the `switch` context and the iret frame — and still needs the iret
  frame for the ring-3 boundary. One frame for everything is fewer concepts.
- **Full register save including segments/x87**: unnecessary — `ds`/`es` are
  kernel constants in both rings here, and FP state is unused.

## Consequences

- The process state is exactly one `IrqFrame` + a PML4 + a kernel stack —
  the same "registers + PC + address space" triple the Mik-64 scheduler used,
  now with real hardware doing most of the save.
- `schedule()` is a pure Rust function over a static table: save the incoming
  frame pointer, pick the next READY slot, set `TSS.rsp0` and `CR3`, return
  the chosen frame pointer. No inline assembly in the scheduler.
- `sys_write`/`sys_exit`/`sys_yield` are syscall numbers on `rax` with the
  arg in `rdi`; `write` returns the same frame, `yield`/`exit` return
  another process's.
- Limitation: `int` is slower than `syscall` (IDT lookup, gate checks, IF
  manipulation). Fine for teaching; ADR candidates exist for a later
  `syscall`/`sysret` migration.
- The `IrqFrame` layout is a contract between `isr.S` push order and the
  `#[repr(C)]` struct — a silent ABI; both sides carry a comment pointing at
  the other.
