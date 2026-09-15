# ADR-005: E820 Memory Map and Free-List Frame Allocator

## Status

Accepted

## Date

2026-09-15

## Context

Milestone 2.2 needs a physical page allocator, and an allocator needs to know
which RAM exists. On a BIOS-booted machine the only source of truth is the
**INT 15h E820** memory map — and BIOS services only exist in 16-bit real
mode, so the map must be collected before `CR0.PE` is set.

Three questions had to be answered:

1. Who collects the map, and where does it live?
2. What happens on the PVH path where no BIOS ran?
3. What allocator structure do we build on top of it?

## Decision

1. **The boot sector collects E820** into a fixed physical buffer at
   `0x5000`: a `'MMAP'` magic u32, a u32 count, then up to 32 standard
   24-byte entries. Real-mode code is the only place BIOS calls work, and a
   fixed address is the simplest possible handoff contract to the kernel.

2. **PVH gets a synthetic fallback map.** If the magic is absent, the kernel
   fabricates two usable regions — `1 MiB..4 MiB` and `__bss_end..128 MiB`
   (QEMU's default RAM size). The fallback keeps the secondary boot path
   working; the primary path always has real data.

3. **A free-list allocator over usable frames.** `free_frame` writes the
   next-pointer into the freed frame itself; `alloc_frame` pops the head —
   O(1) both ways, no backing array, no size cap. `init` pushes every frame
   in every type-1 region except frames below 1 MiB (IVT/BDA, boot sector,
   E820 buffer, stage2 blob all live there) and frames overlapping the kernel
   image `0x400000..__bss_end`.

## Alternatives Considered

### Parse the PVH `hvm_start_info` memmap

- **Pros:** Real memory data on the PVH path too.
- **Cons:** A second parser for a different structure, for a boot path that
  exists only as a comparison. The QEMU-default fallback is honest about its
  assumption and far smaller.
- **Rejected:** Not worth it for a legacy path.

### Bitmap allocator

- **Pros:** O(1) lookup of "is this frame free" without walking; compact for
  known RAM sizes.
- **Cons:** Needs a backing array sized by the RAM ceiling, which reintroduces
  a fixed bound and more code than the free list's 10 lines.
- **Rejected:** The free list matches the Phase 1 progression and needs no
  storage of its own.

### Bump allocator (again)

- **Pros:** Simplest possible.
- **Cons:** Can't free; Phase 1 already outgrew it. Would also need a static
  bound since E820 regions aren't contiguous.
- **Rejected:** The milestone calls for free as well as alloc.

## Consequences

- `boot16.S` grew by ~25 instructions; the disk image format is unchanged
  (the map lives at a physical address, not in the image).
- The kernel sees identical allocator behavior on both boot paths — the only
  difference is where the region list came from, observable in the
  `usable frames=` count.
- SeaBIOS preserves `di` across INT 15h; the boot code relies on that and the
  code comments say so. A hostile BIOS could produce a corrupt map — an
  acceptable simplification under QEMU.
- 32-entry cap: SeaBIOS emits ~7 entries on default QEMU; the cap is generous
  and the parser truncates rather than overflowing.
