# ADR-003: BIOS Disk Boot via Boot Sector and Stage2

## Status

Accepted

## Date

2026-09-15

## Context

Milestone 2.1's x86-64 kernel initially booted only through QEMU's PVH
direct-boot path: a Xen ELF note tells QEMU to load the ELF and jump to its
32-bit entry point with the machine already in protected mode. That is
convenient, but it skips the entire real boot flow — BIOS, boot sector, disk
loading, A20 — which is exactly what this project exists to teach.

The options were:

1. Keep only PVH direct boot.
2. Write a real BIOS boot path: a 512-byte boot sector plus a stage-2 loader.
3. Adopt an existing bootloader such as GRUB or Limine.

## Decision

Implement a real BIOS disk boot while keeping PVH as a comparison path.

- `.boot16` (512 bytes, VMA `0x7C00`) runs in 16-bit real mode. It uses an
  INT 13h EDD LBA read to pull the rest of the image in at `0x7E00`, enables
  A20 via the port-92 fast gate, loads a flat GDT, sets `CR0.PE`, and far
  jumps to stage2.
- `.stage2` (VMA `0x7E00`) runs in 32-bit protected mode. It copies the flat
  kernel image to `0x400000`, zeroes the kernel `.bss`, and jumps to the same
  `_start` the PVH path enters — so the kernel sees identical handoff state
  either way.
- `mik-os-x86`'s runner extracts the two boot sections and the flat kernel
  from the kernel ELF and assembles a 64 KiB raw disk image: sector 0 is the
  boot sector (with the kernel length patched in at offset `0x1F8`), sectors
  1..N are stage2 followed by the kernel.
- `cargo run -p mik-os-x86 -- qemu` boots the disk image; `-- pvh` keeps the
  direct-boot path.

## Alternatives Considered

### Keep Only PVH Direct Boot

- **Pros:** Zero code; QEMU does everything.
- **Cons:** Teaches nothing about how a machine actually boots. The Xen note
  is also a QEMU-specific ABI, not a real firmware contract.
- **Rejected:** Defeats the purpose of the milestone.

### Adopt GRUB or Limine

- **Pros:** Battle-tested; handles A20, memory maps, framebuffer, and more;
  boots on real hardware.
- **Cons:** Hides precisely the mechanics we want to learn. Brings in an
  external dependency and its own build complexity.
- **Rejected:** A learning project should own its boot path; GRUB/Limine can
  be revisited if real-hardware booting becomes a goal.

## Consequences

- The kernel ELF now carries three payloads (`.boot16`, `.stage2`, kernel
  proper); `KEEP()` in `link.x` stops the linker from garbage-collecting the
  boot sections.
- The image builder contains a small ELF section parser rather than shelling
  out to `objcopy`, keeping the toolchain to just Rust + QEMU.
- The disk image is a fixed 64 KiB; the boot sector always reads 127 sectors,
  and the builder errors if the kernel ever outgrows that budget.
- stage2 deliberately reproduces the PVH handoff (32-bit pmode, paging off,
  flat segments) so `boot.S`'s long-mode transition is shared by both paths.
- Verified: QEMU boots the image and prints the serial banner, and
  `tests/boots_in_qemu.rs` automates that check.
