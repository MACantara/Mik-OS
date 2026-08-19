# Spec: Mik-64 Paging / Virtual Memory

## Objective

Add virtual memory (paging) to the Mik-64 virtual machine so the kernel can
manage address spaces. This is the next major OS concept after flat memory and
syscalls, and it directly prepares for the real x86-64 port.

**User stories:**
- As a learner, I want to see a kernel enable paging and continue to run.
- As a learner, I want to see a page fault handled by the kernel.
- As a learner, I want to understand how virtual addresses translate to physical
  addresses through a 4-level page table.

## Design

### Page size

4 KiB (4096 bytes). Page offset = 12 bits.

### Virtual address layout

48-bit virtual addresses. Upper 16 bits (bits 48-63) must be zero. A non-zero
upper 16 bits causes a page fault.

```
| 63..48 | 47..39 | 38..30 | 29..21 | 20..12 | 11..0  |
|  must  | PML4   | PDPT   | PD     | PT     | offset |
|  be 0  | index  | index  | index  | index  |        |
| 16 bit | 9 bit  | 9 bit  | 9 bit  | 9 bit  | 12 bit |
```

Each page table has 512 entries (9 bits), each entry is 8 bytes, so each page
table is exactly 4 KiB = one page.

### Page table entry (PTE) format

Each PTE is a 64-bit word:

```
| 63  | 62..48 | 47..12         | 11..5 | 4  | 3  | 2  | 1  | 0  |
| NX  | reserved| physical page  | avail | D  | A  | U  | W  | P  |
|     |         | number (PPN)   |       |    |    |    |    |    |
```

- Bit 0 — **P** (Present): entry is valid.
- Bit 1 — **W** (Writable): writes are allowed.
- Bit 2 — **U** (User): user-mode code may access. (Privilege is not enforced
  in the MVP; this bit is informational.)
- Bit 3 — **A** (Accessed): set by the CPU when the page is read or written.
- Bit 4 — **D** (Dirty): set by the CPU when the page is written.
- Bits 5-11 — available to the OS.
- Bits 12-47 — **PPN**: physical page number (physical address >> 12).
- Bits 48-62 — reserved, must be zero.
- Bit 63 — **NX** (No Execute): code may not be fetched from this page.

Intermediate (non-leaf) PTEs point to the next-level page table. In that case
the PPN is the physical address of the next table, and the NX bit is ignored.

### Control registers (CSRs)

| CSR number | Name   | Purpose                                    |
|------------|--------|--------------------------------------------|
| 0          | `PTBR` | Physical address of the PML4 root table. Must be page-aligned. |
| 1          | `PMODE`| Paging enable. 0 = disabled (flat mode), 1 = enabled. |

When `PMODE = 0`, all memory accesses use physical addresses directly (the
current behavior). When `PMODE = 1`, all instruction fetches and data loads/
stores go through the page table walker.

### New instructions

| Opcode | Mnemonic  | Format              | Semantics                              |
|--------|-----------|---------------------|----------------------------------------|
| `0x11` | `RDCSR`   | `RDCSR rd, csr_imm` | `rd = CSR[csr_imm]`                    |
| `0x12` | `WRCSR`   | `WRCSR rs1, csr_imm`| `CSR[csr_imm] = rs1`                   |
| `0x13` | `SFENCE`  | `SFENCE`            | Flush the TLB.                         |

`csr_imm` is the 44-bit immediate field, interpreted as the CSR number (only
the low 8 bits are used).

### Page table walk

When `PMODE = 1` and a virtual address `va` is accessed:

1. If `va[63:48] != 0`, raise a page fault (code 4 = non-canonical).
2. `cr3 = PTBR` (physical address of PML4).
3. For each level L from 3 (PML4) down to 0 (PT):
   a. `index = va[12 + 9*L + 8 .. 12 + 9*L]` (9 bits).
   b. `pte_addr = cr3 + index * 8`.
   c. `pte = mem64[pte_addr]`.
   d. If `pte.P == 0`, raise page fault (code 1 = not present).
   e. If L > 0: `cr3 = pte.PPN << 12` (descend to next table).
   f. If L == 0: this is the leaf PTE. Check permissions:
      - Write and `pte.W == 0` → page fault (code 2 = write violation).
      - Instruction fetch and `pte.NX == 1` → page fault (code 3 = NX violation).
      - Otherwise: `pa = (pte.PPN << 12) | va[11:0]`.
   g. Set `pte.A = 1`. If write, set `pte.D = 1`. Write the updated PTE back.
4. Return `pa`.

### Page faults

When a page fault occurs:

- `epc = pc` of the faulting instruction (so `ERET` retries it).
- `x10 = fault_code`.
- `x11 = faulting virtual address`.
- `pc = mem64[0x2010]` (the page-fault vector).

Fault codes:

| Code | Meaning             |
|------|---------------------|
| 1    | Page not present    |
| 2    | Write violation     |
| 3    | NX violation        |
| 4    | Non-canonical address |

The kernel must install a page-fault handler address at `0x2010` before enabling
paging.

### TLB

A small direct-mapped TLB caches recent translations:

- 16 entries, indexed by `va[15:12]` (low 4 bits of the VPN).
- Each entry stores: valid bit, virtual page number, physical page number,
  permission bits.
- Flushed by `SFENCE` and by any `WRCSR` to `PTBR`.
- Flushed by `WRCSR` to `PMODE` (transition to or from enabled).

### Memory map additions

```
0x0000_2000                 : trap vector (syscall handler)
0x0000_2010                 : page-fault vector
```

## Commands

```bash
# Build
cargo build

# Run all tests
cargo test

# Build and run the OS
.\run.ps1
```

## Testing Strategy

- **Unit tests** for the page table walker (translate a VA, check PA).
- **Unit tests** for TLB hit/miss behavior.
- **Integration tests** for:
  - Identity-mapped kernel boots and prints with paging enabled.
  - A page fault is raised and handled.
  - A write to a read-only page faults.
  - An instruction fetch from an NX page faults.
- The existing kernel boot test must still pass (paging disabled path).

## Boundaries

- **Always:** Update `docs/specs/mik-64.md` alongside emulator changes. Write
  tests before implementation. Keep commits atomic.
- **Ask first:** Adding more CSRs or privilege levels beyond PTBR/PMODE.
- **Never:** Break the flat-memory boot path. Existing tests must still pass.

## Success Criteria

- The emulator can translate virtual addresses through a 4-level page table.
- The kernel can set up an identity map, enable paging, and continue to boot.
- A page fault is delivered to the kernel handler with the correct fault code
  and faulting address.
- `cargo test` passes, including new paging tests and all existing tests.
- `.\run.ps1` still boots Mik OS (now with paging enabled).

## Open Questions

- Should we add a global bit to PTEs for kernel pages that are shared across
  address spaces? (Deferred — only one address space in the MVP.)
- Should we add accessed/dirty bit tracking in the TLB or only in the page
  table? (Page table only for now; TLB is a pure cache.)
