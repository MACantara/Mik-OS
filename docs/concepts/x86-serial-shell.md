# x86-64: Serial Console Input and the Shell

This document explains the mechanisms behind the Phase 3 console shell:
how a byte travels from the host keyboard into a ring-3 process, and how
the shell program is built out of the syscalls that already exist.

## The UART as a polled device

COM1 (`0x3F8`) is a 16550 UART. Its registers are accessed with `in`/`out`
instructions at consecutive port addresses:

| Port | Register | Purpose |
|------|----------|---------|
| `0x3F8` | RBR/THR | Receive buffer (read) / transmit hold (write) |
| `0x3FD` | LSR | Line status — bit 0 = "data ready" |

Writing has been fire-and-forget since M2.1 (`outb(0x3F8, byte)`); QEMU
drains the virtual UART instantly. *Reading* has a protocol: a byte typed
on the host arrives in RBR and sets LSR bit 0. `serial::read_byte` polls
that bit; if set it reads RBR (which also clears the bit), if clear it
returns `None`. This is **programmed I/O** — the CPU asks the device, the
device never interrupts the CPU.

QEMU's `-serial stdio` makes the UART bidirectional: bytes written to the
emulator's stdin appear in the VM's RBR exactly as if typed on a real
serial line. That is how the boot test drives the shell in CI.

## `sys_read`: the input half of the syscall boundary

Syscall 7 is deliberately asymmetric with `sys_write`:

- `sys_write` (1): arg in `rdi`, output on the wire. Never fails.
- `sys_read` (7): no args, byte in `rax`, or `u64::MAX` when no data.

Returning the byte in a register — instead of writing through a user
pointer — removes the entire class of "validate that this userspace
address is safe to write" problems. A `read(fd, buf, len)` API must check
`buf` lies inside the private user region, is mapped, and survives a page
fault mid-copy. A one-byte register return checks nothing because it can
touch nothing.

`-1` is an unambiguous sentinel: `read` returns `u64`, and no byte value
`0..=255` equals `u64::MAX`.

## Polling + yield = cooperative blocking

A real `read` would *block* — mark the process `WAITING`, unmask UART
IRQ4, wake the process when a byte lands. The minimal slice gets the same
external behavior without any of that machinery:

```
loop {
    b = sys_read();
    if (b == -1) { sys_yield(); continue; }
    handle(b);
}
```

The shell asks, finds nothing, and donates the rest of its 10 ms slice.
The round-robin scheduler rotates through A, B, and the shell; the shell
is just a process that happens to do very little work per dispatch.
Latency is bounded by the tick: worst case ~30 ms for a typed byte to be
consumed (three slots, one tick each) — invisible to a human, free to
implement.

## The shell as an ordinary process

`prog_sh` is assembled into the kernel image like the other programs,
`spawn`ed third, and scheduled identically. Two of its properties are
worth noting:

- **It may use `call`/`push`/`ret`.** Its stack page is privately mapped
  writable — it was `spawn`ed directly, never produced by `fork`, so no
  COW read-only page sits under `rsp`. That is why the string printer can
  be a real subroutine instead of inlined loop code. (The forked child
  shares this constraint only until its first stack write faults a
  private copy in.)
- **`q` is final.** `sys_exit` marks the slot `DEAD`; nothing re-spawns
  the shell. In a real init system, PID 1 exiting is a kernel panic or a
  respawn — here it is simply the end of interactivity, and B keeps
  ticking.

## Data flow, end to end

```
host stdin -> QEMU -serial stdio -> UART RBR + LSR bit 0
   -> inb(0x3FD)&1, inb(0x3F8)        [serial::read_byte, ring 0]
   -> rax in the IrqFrame             [syscall_handler, ring 0]
   -> iretq                           [ring 3, byte in rax]
   -> sys_write echo                  [ring 3 -> ring 0 -> COM1 THR]
   -> QEMU stdio -> host stdout
```

Every crossing in that path already existed — `inb` since M2.1, the frame
ABI since M2.3, scheduling since M2.3, user stacks since M2.4. The shell
added exactly one new mechanism (the LSR poll) and one new syscall.
