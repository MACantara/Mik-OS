# Concepts: x86-64 Memory Management

What Milestone 2.2 implements, mechanism by mechanism — the map from "BIOS
tells us what RAM exists" to "a second address space runs code".

## E820: asking the BIOS what RAM exists

An OS cannot guess where RAM is: the physical address space has holes for
ROM, MMIO apertures, and firmware data. The BIOS answers the question via
**INT 15h, function E820h** — callable only in 16-bit real mode:

- `ES:DI` points at a 24-byte output buffer, `EBX` is a continuation token
  (start at 0, the BIOS hands back the next value), `ECX` = buffer size,
  `EDX` = `'SMAP'` magic, `EAX` = `0xE820`.
- Each call returns one entry: `base`, `length`, `type` (1 = usable RAM,
  others = reserved/ACPI/reclaimable), `acpi` flags.
- `EBX` = 0 after the last entry; `CF` on the first call means unsupported.

`boot16.S` loops it into a fixed buffer at physical `0x5000` (magic + count +
entries) so the kernel can parse it after entering long mode. On QEMU with
default 128 MiB the map has ~7 entries: a usable low-memory chunk, the
EBDA/BIOS-ROM reserved holes below 1 MiB, and one big usable region covering
most of RAM.

## The free-list frame allocator

Physical memory is managed in 4 KiB **frames**. The allocator is a singly
linked list threaded through the free frames themselves:

```
free_frame(pa):  *(u64*)pa = head; head = pa;      // O(1)
alloc_frame():   pa = head; head = *pa; return pa;  // O(1)
```

No bitmap, no counters, no size cap — the memory being managed *is* the
bookkeeping structure. `init` walks every usable E820 region and pushes each
aligned frame, minus two exclusion ranges: everything below 1 MiB (IVT, BDA,
boot sector, the E820 buffer itself, the stage2 blob) and the kernel image
`0x400000..__bss_end` (code, page tables, stack).

This is the same structure Phase 1's Mik-64 kernel used — the concept ports
verbatim; only the source of "which frames exist" changes.

## Four-level paging on x86-64

Every virtual address is translated through four 512-entry tables:

```
CR3 -> PML4 -> PDPT -> PD -> PT -> page
        512GB   1GB    2MB   4KB     per entry
```

VA bits: `[47:39]` index PML4, `[38:30]` PDPT, `[29:21]` PD, `[20:12]` PT,
`[11:0]` page offset. Each entry is a physical address (bits 12+) OR-ed with
flags:

- `P` (bit 0) — present; absent entries fault
- `W` (bit 1) — writable; else writes fault
- `U` (bit 2) — user-accessible; **must be set at every level** for ring 3
- `PS` (bit 7) — page size: in a PD entry it means a 2 MiB leaf, skipping PT

The kernel's boot-time map is a PML4->PDPT->PD chain of 2 MiB `PS` entries
identity-mapping the low 1 GiB — VA == PA, which is why physical addresses
work as pointers everywhere in the kernel.

`map_4k` is the dynamic counterpart: it walks the three upper levels for a
VA, allocating+zeroing a table when an entry is absent, then writes the leaf
PTE. Demand paging in M2.4 is "call `map_4k` from the page-fault handler".

## CR3 and address spaces

`CR3` holds the physical address of the active PML4 — it *is* the address
space. Two PML4s that share an entry share everything below it. Our second
address space exploits that:

```
kernel pml4[0] -> kpdpt[0] -> pd -> 1 GiB identity (2 MiB pages)
user   pml4[0] -> updpt[0] ----^   (shared: kernel keeps running)
                 updpt[1] -> upd -> upt -> user frame @ 0x40000000
```

`updpt[0]` points at the kernel's PD, so the whole identity map — kernel
code, stack, allocator state — exists in both spaces. `updpt[1]` is private:
`0x40000000` maps the user blob under the user CR3 and is *unmapped* under
the kernel CR3. `mov cr3, up4` switches; `call 0x40000000` runs it (`U` on
COM1); `mov cr3` back. Same instruction stream, two different views of
memory — the essence of a process address space.

## The ordering constraint that bit us

The free list writes a link into every frame it adopts — which means those
frames must be **mapped** before `init` runs. The boot tables only cover
6 MiB, while usable RAM extends to 128 MiB. So the sequence is:

1. `extend_identity_map()` — fill `pd[3..512]` in the already-mapped `.bss`
   tables, reload CR3 (also the TLB flush).
2. `mem::init()` — now free-frame writes to any address are safe.
3. Allocate table/user frames — everything is reachable.

Getting this backwards was the first bug the M2.1 IDT ever caught: `EX0E`,
a page fault, reported instead of a silent triple fault.
