# Why Device Drivers Matter

Milestone 3.2 is the first time Mik OS talks to hardware it did not
invent. The serial port from earlier milestones was a debug channel we
controlled completely; a keyboard, a UART IRQ, a VGA card, and the PCI
bus are the kernel meeting *someone else's* hardware on *their* terms.
That shift teaches things no amount of CPU-side scheduling code can.

## It is where "the kernel" meets physics

Every previous subsystem was self-authored: we defined the IrqFrame, the
syscall numbers, the page-table layout. Devices don't care about our
conventions. The UART asserts IRQ4 on its schedule; the keyboard speaks
scancodes, not ASCII; PCI config space exists at fixed ports whether or
not the kernel is ready. A driver is the translation layer between the
hardware's protocol and the kernel's abstractions — and writing one is
where "operating system" stops being a euphemism for "a program that
calls itself".

## Interrupts are the point of the whole exercise

Until now, IRQs only *took* CPU time (the timer preempting processes).
M3.2 shows the other half: interrupts as the mechanism that makes I/O
*cheap*. The shell now sleeps in `hlt` while waiting for a keypress and
the CPU runs other processes — versus M3.1, where waiting meant burning
slices to check a flag. Polled I/O vs. interrupt-driven I/O is the
difference between a CPU that waits and a CPU that works, and it is
impossible to appreciate the second until you have watched the first
waste a timeslice a hundred times a second.

The flip side — an edge-triggered IRQ can be missed, coalesced, or never
delivered by a quirky emulator — is exactly why real drivers pair the
interrupt with a periodic poll. `drain_rx()` in the timer tick is 5 lines
that exist because we watched QEMU's Windows stdio eat a burst: driver
bugs are found by watching, not by reading the spec harder.

## Blocking I/O is where the scheduler earns its name

`WAITING` is the first process state that isn't "ready" or "dead" — the
first time a process says "I cannot run, don't schedule me." The
implementation (rewind `rip` over `int 0x80` so the syscall replays on
wake) is three lines, but it is the *real* mechanism — the same
restartable-syscall trick Linux uses (`ERESTARTSYS`) so `read()` can
sleep without corrupting user state. And the idle loop that parks the CPU
when *everything* is blocked is the kernel's honest answer to "what runs
when nothing can": the answer is nothing — the CPU halts until hardware
says otherwise.

## Enumeration before drivers

PCI config space exists because "what hardware is attached" is itself a
question the kernel must answer. The 65k-probe scan is the simplest
correct enumerator — and its output is already a map of the next two
milestones: the IDE controller (`class 0101`) for the file system, the
e1000 (`class 0200`) for networking. Learning to *find* devices before
learning to *drive* them mirrors how real kernels boot: enumerate, match,
attach.

## The line every OS walks

`sys_write` now writes to *two* consoles — the kernel log (COM1) and the
user screen (VGA). That tiny duplication is the seed of console policy:
which bytes go where, who owns the display, what happens when two
processes write at once. Meanwhile the shell buffers lines in *user*
space — the kernel stays a byte pipe. Both choices echo the perpetual OS
question: what belongs in the kernel (the mechanism) and what belongs
outside it (the policy). M3.2 keeps the kernel minimal and puts policy
in the process that needs it — which is the answer that scales.
