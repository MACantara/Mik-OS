# Concepts: x86 Boot Chain and the IDT

What Milestone 2.1 actually implements, mechanism by mechanism. This is the
map from "power on" to "Rust code handling exceptions" for the x86-64 port.

## The BIOS boot chain

A PC starts in 16-bit **real mode** executing the BIOS. The BIOS reads sector
0 of the boot disk into physical address `0x7C00`, checks for the `0xAA55`
signature at bytes 510–511, and jumps to it with `DL` = the boot drive.

Our `.boot16` sector then does the minimum real boot work:

1. **Sets up segments and a stack** — real mode addresses are
   `segment * 16 + offset`, so a sane `DS`/`SS`/`SP` must come first.
2. **Loads the rest of the image with INT 13h AH=42h** — the BIOS's own disk
   service. A Disk Address Packet describes an LBA read; we always pull 127
   sectors (the image is padded to 64 KiB) to `0x7E00`, right after the boot
   sector. This is the only realistic way to load more than 512 bytes without
   implementing an ATA/NVMe driver.
3. **Enables A20 via port 0x92** — a legacy quirk: address line 20 was once
   masked for 8086 compatibility, so addresses above 1 MiB wrap unless the
   "fast A20 gate" bit is set.
4. **Enters protected mode** — load a flat GDT (`lgdt`), set `CR0.PE`, far
   jump to flush the CPU into 32-bit decoding.

## Stage2 and the handoff contract

`.stage2` runs in 32-bit protected mode and is a pure loader:

- `rep movsb` copies the flat kernel image (which follows stage2 in the
  loaded blob) to its link address `0x400000`.
- `rep stosb` zeroes the kernel `.bss` — the ELF carries no bytes for it, and
  our page tables and stack live there.
- It jumps to `_start`, delivering **exactly** the state QEMU's PVH entry
  delivers: 32-bit protected mode, paging off, flat segments.

That shared contract is the design's key simplification: `boot.S` needs one
entry point, not two.

## The long-mode transition (in `boot.S`)

From 32-bit protected mode to 64-bit long mode:

1. Build page tables: a PML4 → PDPT → PD chain identity-mapping the first
   6 MiB with 2 MiB pages.
2. `mov cr3, pml4` — point the MMU at the tables.
3. Set `CR4.PAE` — required for long-mode paging.
4. Set `EFER.LME` (MSR `0xC0000080`) — "long mode enable".
5. Set `CR0.PG` — paging on; the CPU is now in compatibility mode and wants
   64-bit code.
6. Far return/jump through a 64-bit code descriptor in `gdt64` — this is what
   actually flips the CPU into 64-bit mode.
7. `kmain` (Rust) prints the banner on COM1.

## The IDT

The **Interrupt Descriptor Table** is an array of 16-byte **gate
descriptors**; `lidt` loads its base/limit. When an exception or interrupt
fires, the CPU uses the vector number as an index, reads the gate, and jumps
to `selector:offset` after switching stacks per its rules.

Our gate descriptor fields, concretely:

- `offset` — the handler address (`isr_N` stubs).
- `selector = 0x08` — the `gdt64` code segment.
- `flags = 0x8E` — present, DPL 0, **interrupt gate** (clears IF on entry, so
  handlers are uninterruptible; contrast trap gates, which keep IF).
- `ist = 0` — no alternate stack yet (needs TSS; a later milestone).

The handler side is deliberately panic-style: each stub `push`es its vector
number and jumps to `isr_common`, which prints `EX` + two hex digits and
halts. Uniform stubs work *because* handlers never return — a resumable
handler would additionally need to account for the CPU-pushed error code that
some vectors (8, 10–14, 17) add to the frame.

## How the pieces fit on disk

```text
sector 0        .boot16   BIOS -> 0x7C00 (kernel_len patched at 0x1F8)
sector 1..      .stage2   loaded at 0x7E00 by INT 13h
                kernel    flat image, copied to 0x400000 by stage2
```

The kernel ELF holds all three payloads in separate sections; `KEEP()` in the
linker script keeps the unreferenced boot sections, and `mik-os-x86`'s image
builder flattens them into the raw disk image.
