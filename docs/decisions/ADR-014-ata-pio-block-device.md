# ADR-014: ATA PIO as the Block Device

## Status

Accepted (Milestone 3.3).

## Context

The filesystem milestone needs a real block device — the acceptance
criterion is that a file written through the syscall API survives a QEMU
restart, which rules out a RAM disk. The candidates:

- **ATA PIO** on the primary bus (ports 0x1F0-0x1F7/0x3F6): QEMU's default
  `-drive` lands on the PIIX3 IDE controller in legacy mode — the same
  disk that booted us, with fixed ports and no BAR setup.
- **virtio-blk** (`-device virtio-blk-pci`): the modern protocol — PCI
  BAR negotiation, feature bits, and three chained virtqueue descriptors
  per request. Cleaner long-term, meaningfully more machinery.
- **ATA with DMA / IRQ14**: interrupts instead of polling. More code for
  no pedagogical gain at this size.

## Decision

ATA PIO, LBA28, one sector at a time, fully polled (`ata.rs`):

- `read_sector` / `write_sector` wait for `BSY` to clear and `DRQ` to set,
  then move 256 words with `rep insw` / `rep outsw`.
- Every write ends with **FLUSH CACHE** (0xE7) so the data reaches the
  host image file before the syscall returns — that, not the sector
  write, is what "persistence" means under an emulator.
- `init` probes by reading sector 0 and checking the 0xAA55 boot
  signature; a `PRESENT` flag lets the PVH path (which can run without a
  disk) degrade gracefully instead of hanging a nonexistent device.

No IRQ14, no DMA, no multi-sector commands, no IDENTIFY parsing. Polling
is correct here because all callers already run inside interrupt gates
on a single CPU — nothing else could use the cycles anyway.

## Consequences

- ~120 lines for a working, durable block device.
- Disk I/O is synchronous and CPU-bound; the caller spins in `BSY`/`DRQ`
  loops. Fine at this scale — the upgrade path (IRQ14 + a wait queue,
  or virtio-blk + DMA) is a later-milestone concern and doesn't change
  the `read_sector`/`write_sector` interface the FS builds on.
- Mik-FS lives at sector 256+, past the 128 KiB kernel load area; the
  image was padded to 1 MiB so the FS region always exists.
