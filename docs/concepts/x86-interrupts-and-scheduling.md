# x86-64: Interrupts, Privilege Transitions, and Scheduling

Milestone 2.3 wires the last pieces needed for a preemptible OS: a GDT/TSS
that gives the CPU a ring-0 stack to land on, a PIC/PIT pair that generates a
periodic interrupt, an IDT that turns both hardware IRQs and `int 0x80` into
the same stack-frame shape, and a round-robin scheduler that is just a
function returning a frame pointer. This is the same model Phase 1 built on
Mik-64 (`TRAP`/`IRET` + `CSR_TIMER`), mapped onto real hardware.

## The privilege boundary

Ring 3 (user) and ring 0 (kernel) are privilege levels enforced by segment
descriptors. A ring-3 `cs` has RPL=3; while it is loaded, the CPU refuses
access to pages whose PTE lacks `U` — at *every* level of the page walk
(`PTE_U` must be set on the PML4E, PDPTE, PDE, and PTE, a rule that cost us
a debugging round: a user page whose PML4 entry lacked `U` faulted despite
all four levels being present). Privileged instructions (`hlt`, `out`,
`lgdt`, `iretq`... `int` is fine — it is the sanctioned door) and port I/O
without an IOPB entry `#GP`-fault in ring 3.

The GDT for this needs four code/data descriptors: kernel code/data
(`0x08`/`0x10`) plus user data/code (`0x1B`/`0x23` with DPL=3). The TSS
exists almost solely for `rsp0`: the kernel-stack pointer the CPU loads
automatically on a ring-3 → ring-0 transition. Per-process kernel stacks
work by rewriting `TSS.rsp0` at every context switch.

## The iret frame — the currency of everything

When an interrupt, exception, or `int` arrives while in ring 3, the CPU
pushes `ss, rsp, rflags, cs, rip` on the new stack. `iretq` pops exactly that
and continues — possibly into a different privilege level and stack. That
five-word frame **is** a resumable execution context; our `IrqFrame` adds the
15 GPRs the stub pushes, giving one struct that fully describes a stopped
process: registers + iret frame. The scheduler's context switch is:

```
save:   PROCS[cur].frame = incoming_rsp
pick:   next = next READY slot
switch: TSS.rsp0 = next.kstack_top; CR3 = next.pml4
return: next.frame               // irq_tail pops it, iretq resumes it
```

First entry into ring 3 fabricates the same frame by hand (`enter_user`) —
no separate "jump to user mode" mechanism is needed because `iretq` already
knows how to lower privilege.

## Interrupt gates, DPL, and IF

An IDT gate's type decides whether hardware interrupts may fire inside the
handler: our gates are interrupt gates (`0x8E`), which clear `IF` on entry —
the scheduler and syscall handler run uninterruptibly by construction. The
gate's DPL decides which rings may raise it *by software*: `int 0x80` needs
DPL=3 (`0xEE`) or ring-3 `int` would `#GP`. Hardware IRQs ignore the DPL
field — the PIC can always vector in.

## PIC + PIT — how the tick reaches the CPU

The 8259 PIC pair arbitrates hardware IRQ lines onto the CPU's INTR pin; by
default IRQ0–7 land on vectors 8–15, colliding with exceptions, so both PICs
are remapped to 32–47 via ICW1–4 port writes, then OCW1 masks everything
except IRQ0. The 8254 PIT's channel 0 output is wired to IRQ0; a divisor of
11931 on the 1.193182 MHz base gives ~100 Hz. Each IRQ must be acknowledged
(EOI, `out 0x20, 0x20`) — before scheduling in our handler, so a tick can
arrive on the *new* process's stack next time.

Two real-hardware footnotes this milestone learned:

- **BIOS leaves the PIT ticking** (~18.2 Hz). IRQ0 stays latched while `IF=0`
  and delivers the moment the first `iretq` enables interrupts — so
  `sched::start` drains it under a "scheduler inactive" flag before the first
  dispatch.
- Exceptions from ring 3 also land on `TSS.rsp0` — the same stack the
  scheduler swaps — which is why the exception stubs only ever print+halt:
  a faulting user process has no resume story yet (that is M2.4's job).

## The demo

`prog A` = `write('A')`, `yield`, `write('a')`, `yield`, `write('A')`,
`exit`. `prog B` = `write('B')`, then `jmp $` — it never yields, so every
departure from B is a timer preemption. Serial shows `ABaA`:

```
A  -> write 'A', yield
B  -> write 'B', spin; 100 Hz tick -> isr_timer -> schedule -> A
A  -> write 'a', yield
B  -> spin; tick -> A
A  -> write 'A', exit -> DEAD -> B idles forever
```

Both `sys_write` (to COM1) and `sys_exit` are `int 0x80` with `rax` =
number, `rdi` = arg.
