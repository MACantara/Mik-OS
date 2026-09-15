# Why the Boot Chain and the IDT Matter

The M2.1 mechanisms are easy to dismiss as ceremony — "just get to `kmain`."
They are not. Each one is a load-bearing concept that the rest of the OS is
built on. This is the "so what" for each.

## The boot sector is the trust boundary you cannot skip

Every x86 machine in the world starts at the same place: 16-bit real mode,
one 512-byte sector, `0xAA55`. There is no configuration to bypass it — the
firmware contract *is* the platform. Writing `.boot16` teaches the first real
lesson of systems work: **your program does not get to choose its entry
state.** The kernel has to climb out of a 1981 execution environment using
nothing but the firmware's own services (INT 13h) before it can even see the
memory it will manage.

That constraint also explains real-world structure: GRUB, `bootmgfw`, and
every other bootloader are multi-stage precisely because 512 bytes is not
enough to do anything but load the next stage. Our `.stage2` is that pattern
in miniature.

## A20 and the GDT are the price of compatibility

The A20 gate exists because the 8086 wrapped addresses at 1 MiB and software
came to depend on it. Toggling a bit on port `0x92` to unmask an address line
is the clearest possible demonstration that **real hardware carries its
history as live behavior**, not as documentation. The GDT is similar: segment
descriptors are nearly vestigial in long mode, yet the CPU still requires a
valid one to change modes — you cannot reach 64-bit code without it.

## The long-mode transition is paging before paging

Setting `CR4.PAE` → `EFER.LME` → `CR0.PG` → far jump is the *first* time the
x86 port touches the machinery Phase 1 explored on Mik-64: a page-table
walk (`cr3` ↔ `PTBR`), an enable bit (`CR0.PG` ↔ `PMODE`), and identity
mappings chosen so the executing code doesn't fault mid-transition. The
2 MiB-page identity map here is the same "map it before you need it" move the
Mik-64 kernel made with its kernel pages — on real hardware this time.

## The IDT is how the CPU talks back

Until the IDT exists, the kernel is deaf: a bad memory access, a divide by
zero, a breakpoint — all triple-fault into an instant reset with no
explanation. `EX03` on the serial line is the difference between *debugging*
and *divining*. Every later milestone depends on it:

- **M2.3's scheduler** is a timer interrupt — vector 32+ — plumbed through
  this same table with a resumable stub.
- **System calls** on x86-64 can arrive through the IDT (`int 0x80`-style) as
  well as `syscall`/`sysret`.
- **Page faults** (vector 14) are the hook demand paging needs — the exact
  mechanism Phase 1 used on Mik-64, delivered here through `isr_14`.

The print-and-halt policy is also a concept, not a shortcut: a **panic
handler** is the kernel's last resort — the thing that runs when continuing
would corrupt state. The Mik-64 kernel's `F<code>` halt was the same idea in
miniature.

## The shared handoff contract is the portability lesson

PVH and BIOS are two completely different boot paths that converge on one
contract — 32-bit pmode, paging off, flat segments, jump to `_start`.
Designing stage2 to *reproduce* the PVH state rather than invent a second
entry convention is the same discipline real kernels use (Linux has a
handful of boot protocols converging on common setup code). One kernel, many
boot paths: that is what makes the rest of the kernel boot-agnostic.
