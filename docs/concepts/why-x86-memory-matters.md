# Why x86-64 Memory Management Matters

M2.2 looks like plumbing — read a table, push some pointers, write a
register. Each piece is actually a concept the rest of the OS cannot exist
without. The "so what" for each:

## E820 is how an OS learns what it owns

Between "the machine has RAM" and "the OS can use it" sits the memory map.
RAM is not a flat array — firmware, ROMs, and MMIO apertures punch holes in
the physical address space, and writing into a reserved hole can corrupt the
BIOS or hit a device register. E820 is the firmware's answer to "which bytes
are safe", and real-mode collection is a hard constraint: once `CR0.PE` is
set the BIOS is gone forever. Every real bootloader does exactly this dance;
Linux's `e820__` subsystem still parses the same structure our boot sector
writes. Skipping it isn't simpler — it's guessing, and the first allocator
that hands out a firmware-owned frame produces corruption that surfaces
days later, nowhere near the cause.

## The free list is the moment memory becomes a resource

Until now every address was a constant. The allocator is where "address"
becomes "something you can run out of, hand out, and get back". Two design
lessons live in ten lines of code:

- **The resource manages itself.** Free frames hold their own list links, so
  bookkeeping costs zero memory — a trick real allocators still use.
- **Exclusion is policy.** The skip ranges (< 1 MiB, kernel image) are the
  difference between an allocator and a memory corruption generator. The
  Mik-64 kernel needed a hand-picked `0x710000`; here the E820 map plus two
  ranges express the same policy against real, irregular hardware.

Everything after this point — page tables, user processes, COW, file caches —
spends frames from this list.

## Page tables in Rust mean the kernel owns its own view

The boot.S tables were scaffolding: three static arrays that got us to long
mode. Rebuilding/extending them from `mem.rs` is the milestone's real point —
**the kernel now manipulates the structure that defines what it can see.**
`map_4k` is the single primitive behind demand paging (M2.4: call it from the
fault handler), `fork` (map a child copy), device mapping, and kernel
self-protection. Until the kernel can write PTEs, it is a guest of whatever
the bootloader set up; after, it is the owner.

The 2 MiB-vs-4 KiB split is also the concept real systems live by: huge
pages for the cheap bulk map, 4 KiB granularity where per-page control
matters.

## CR3 is the cheapest magic trick in the OS

`mov cr3, reg` — one instruction — swaps the entire meaning of every address.
The M2.2 demo is deliberately small (one page, one `call`, one letter on the
UART) because the *mechanism* is the lesson: process isolation on real
hardware is not walls between programs, it's two translations of the same
numbers. `0x40000000` is a user page under one CR3 and a page fault under the
other; nothing moved, nothing was copied — only the root pointer changed.

That single register is also why the shared-PD layout matters: the kernel
keeps running after the switch only because its code is mapped identically in
both spaces. Every OS pays this same tax (Linux's shared kernel PGD entries,
Windows' system address space) — we've just done it by hand, where the cost
is visible.

## The EX0E bug is the reason the IDT came first

The ordering bug in this milestone — pushing free-list links into unmapped
frames — produced `EX0E` on the serial line: vector 14, page fault,
identifiable and debuggable in minutes. Before M2.1's IDT, the identical
mistake would have been a silent triple-fault reset with no clue why. This is
the real payoff ordering: each milestone's mechanism becomes the next
milestone's debugging tool.
