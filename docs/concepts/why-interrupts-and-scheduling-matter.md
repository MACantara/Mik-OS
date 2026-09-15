# Why Interrupts, Privilege Transitions, and Scheduling Matter

Phase 2.3 is where the x86 kernel stops being a monitor program and starts
being an operating system. Each mechanism maps to a property the OS is
supposed to guarantee.

## The TSS/rsp0 boundary is where trust is enforced

Everything the OS protects rests on one mechanism: when ring 3 traps, the
CPU *unconditionally* swaps to a kernel-chosen stack (`TSS.rsp0`) before a
single handler instruction runs. User code cannot influence where the kernel
lands or what runs next — the hardware does it. Without that atomic stack
switch, a malicious or buggy program could force the kernel to handle its
trap on a user-controlled stack and win immediately. `rsp0` is the concrete
answer to "how does the kernel stay in control when the CPU was running
untrusted code an instruction ago?"

## The iret frame is the minimum viable checkpoint

Preemption, syscalls, and exceptions all reduce to the same question: can
you stop a computation and restart it bit-for-bit later? x86's answer is the
five-word iret frame — `rip` (where), `cs`/`ss` (who/which ring), `rflags`
(mode), `rsp` (stack). Add GPRs and `CR3` and you have *the whole process*;
nothing else about a stopped process matters. This is why the same
`IrqFrame` works for first dispatch, syscall return, and timer switch: if
your context record is complete, "resume" is one `iretq`. OS concepts like
"process," "blocked," and "runnable" are bookkeeping labels over this
checkpoints — the hardware primitive is the frame.

## The timer interrupt is the difference between a kernel and a library

`prog B` spins forever and still gets descheduled. That single fact is what
separates an OS from code that politely borrows the CPU: **preemption**. No
cooperation from the process is required — the PIT fires, the PIC vectors it,
the IDT gate lands on a handler, and the scheduler decides who runs next.
Isolation and fairness both rest on it: one process can neither starve the
others (round-robin slices) nor observe their memory (`PTE_U` at every level,
private PD). Losing the tick mechanism means losing every guarantee a
multi-process OS makes.

## `int 0x80` shows syscalls are just interrupts you meant to raise

A system call is not a function call — it is a controlled privilege
escalation through a gate the kernel published (vector 0x80, DPL=3). The
program asks, the hardware checks, the kernel decides. This is also why the
gate's DPL exists: `int 0x80` is callable from ring 3 while `int 0x20` (the
timer vector) is not — a user program cannot *pretend* to be the tick.

## Why this milestone is the hinge of the roadmap

Everything before it is setup; everything after it builds on this exact
machinery:

- **M2.4 (demand paging, fault handling)**: the page-fault handler is
  another frame-producing entry; `ERET`-style retry is just `iretq` back to
  the same `rip`.
- **M2.5 (`fork`/`exec`/`wait`)**: fork = clone an `IrqFrame` + address
  space; exec = fabricate a fresh frame — both are already in the vocabulary
  this milestone created.
- **SMP / drivers later**: per-CPU `rsp0`s and IRQ1+ unmasking are
  extensions of the same PIC/TSS plumbing.

The costliest lessons were also the most portable: U/S is checked at *every*
page-walk level, and hardware state (the BIOS PIT) outlives the boot that
created it. Both are general OS truths this milestone made concrete.
