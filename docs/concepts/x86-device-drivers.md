# x86-64: Device Drivers — PCI, Interrupts, and Blocking I/O

Milestone 3.2 turned the console from "one serial port the kernel writes
to" into a real device-driver stack. This document explains the four
mechanisms it introduced.

## 1. PCI configuration space — how the kernel finds hardware

Every PCI function exposes a 256-byte configuration header holding its
vendor ID, device ID, class code, BARs, and interrupt line. The CPU reads
it through two I/O ports:

```
outl(0xCF8, 0x8000_0000 | bus<<16 | dev<<11 | func<<8 | reg*4);
value = inl(0xCFC);
```

A vendor ID of `0xFFFF` means "no such device". `pci::scan` walks every
bus/device/function and prints what answers — on QEMU's default machine
that is the i440FX host bridge, the PIIX3 ISA/IDE/ACPI trio, the VGA card,
and the e1000 NIC. Enumeration is how real kernels decide which drivers
to load: match on `vendor:device` or on the class code (e.g. `0101` = IDE
controller, `0200` = Ethernet, `0300` = display).

## 2. Interrupt-driven input — the device calls the kernel

Polled I/O (`inb` in a loop) wastes CPU and adds latency. The interrupt
path inverts control:

- **UART IRQ4**: `serial::enable_rx_irq` sets the 16550's IER bit 0 and
  enables its 16-byte FIFOs (FCR). Each received byte raises IRQ4 →
  `isr_uart` → `uart_handler` drains RBR into the input ring buffer.
- **Keyboard IRQ1**: each scancode byte at port `0x60` raises IRQ1 →
  `isr_kbd` → `kbd_handler` decodes the set-1 make code (two tables —
  unshifted/shifted) and pushes ASCII.

Both end the same way: `input::push` + `sched::wake_on_input()` + PIC EOI.
The handlers never reschedule — they make work *available*; the timer's
round-robin picks it up. That separation (interrupt = "make runnable",
timer = "choose who runs") is the standard minimal discipline.

A subtlety that bit us: the PIC is **edge-triggered** while UART RX is
level-asserted. A byte arriving during an IF=0 window can coalesce its IRQ
with an earlier one, and emulated UARTs have delivery quirks (QEMU's
Windows stdio chardev holds bursts host-side). `timer_handler` therefore
also calls `drain_rx()` — an interrupt fast path with a polling safety
net, a pattern real drivers use (NAPI on Linux is its descendant).

## 3. Blocking `sys_read` — the restartable syscall

M3.1's read returned `-1` on empty and the shell yield-polled. Now:

```text
sys_read on empty buffer:
    proc.state = WAITING
    frame.rip -= 2        // int 0x80 is CD 80 — resume re-executes it
    schedule()            // run someone else

ISR pushes a byte:
    every WAITING proc -> READY

woken proc dispatches -> iretq to the int 0x80 -> re-trap ->
sys_read pops the byte -> rax = byte -> resume
```

The process never sees that it slept. This is the textbook
`ERESTARTSYS` mechanism: a blocking syscall is "return an error asking
userland to retry" or, here, "rewind the PC so the trap replays itself".
It works because the `IrqFrame` *is* the whole process state — there is
nowhere else a partial syscall could hide.

No locks appear anywhere in the input path: producers run in interrupt
gates, the consumer in the `int 0x80` gate, all with IF cleared. On a
single CPU, interrupt gates are the lock.

## 4. The all-waiting edge — kernel idle

If every process is `WAITING` or `DEAD`, `schedule()` parks in
`sti; hlt` until an ISR marks someone `READY`. The `IN_IDLE` gate drops
timer ticks fired inside that loop — a tick there would save a useless
idle-loop frame over the blocked process's carefully rewound one. On real
hardware this loop is where `hlt` saves power; here it mostly documents
that "nothing to do" is a legitimate scheduler state.

## 5. VGA text mode — the cheapest real display

The text console is a raw memory region at physical `0xB8000`: 80×25
cells, each `char + attribute` (2 bytes). Writes are memory stores — no
protocol, no handshake. `vga.rs` keeps a cursor index, expands `\n` to
"next row start", handles backspace, and scrolls by `memmove`-ing rows up.
`sys_write` mirrors to COM1 + VGA, so serial-based tests and the QEMU
stdio workflow keep working while the VGA window becomes the user's
screen.

## 6. Line discipline lives in user space

The kernel still delivers a byte stream. The *shell* buffers until Enter
(either `\r` or `\n`), echoes as typed, and erases on backspace with the
`\b \b` triple (works on both VGA and a serial terminal). This keeps the
syscall ABI pointer-free — `sys_read` returns a byte in `rax`, no user
buffer validation needed. A kernel-side canonical tty is the deferred
upgrade; for one shell, user-side discipline is the minimum that works.
