# ADR-006: Address-Space Layout — Shared Kernel PD, Private User Region

## Status

Accepted

## Date

2026-09-15

## Context

Milestone 2.2 requires `CR3` switching to a first process address space. The
design question is how a second PML4 should relate to the kernel's: which
entries are shared, which are private, and where user mappings live.

Constraints from the existing design:

- The kernel is identity-mapped (VA == PA) in the low 1 GiB via a single PD
  of 2 MiB pages.
- There is no higher-half kernel yet — kernel code must keep running after
  the CR3 switch, so the user table must map it identically.
- Ring 3 does not exist yet (M2.3); the demo executes the user page from
  ring 0, which is legal because SMEP/SMAP are not enabled.

## Decision

A user address space is a fresh PML4 whose PDPT **shares the kernel's PD**
(covering the identity-mapped low 1 GiB, supervisor-only) and gains a
**private PD** at PDPT index 1 covering `0x40000000..0x80000000` for user
pages.

- Shared: `updpt[0] -> pd` — kernel image, stack, allocator structures, and
  the E820 buffer work unchanged under either CR3.
- Private: `updpt[1] -> upd -> upt -> user frames` — user mappings at
  `0x40000000+` exist only in the user table.
- The demo maps a position-independent blob (`out` 'U' to COM1 + `ret`) at
  `0x40000000`, switches CR3, `call`s it, and switches back.

This mirrors the Mik-64 shape from Phase 1: shared lower-level table entries
for the kernel, a private table for the user region.

## Alternatives Considered

### Share the PDPT itself (`up4[0] = kp4[0]`)

- **Pros:** Fewer frames, less code.
- **Cons:** The user PD would be installed into the *shared* PDPT, so the
  kernel PML4 would see user mappings too — the "separate" address space
  would not be separate. Worse, a private 4 KiB user page at `0x800000`
  would need to split the kernel's shared 2 MiB identity PD entry.
- **Rejected:** Doesn't actually create a second address space.

### Full copy of kernel mappings into private tables

- **Pros:** Complete isolation; no shared structures.
- **Cons:** Copies the whole PDPT/PD per process; every future kernel mapping
  change must be re-synced into every address space.
- **Rejected:** Sharing upper levels is what real kernels do (Linux shares
  the kernel half of every PGD); copying buys isolation we don't need yet.

### Higher-half kernel + user at low VAs

- **Pros:** The conventional final layout; user programs get low addresses.
- **Cons:** Requires relocating the kernel image to a high-half link address
  and remapping — a large change orthogonal to this milestone.
- **Rejected:** Deferred; noted as the upgrade path.

## Consequences

- `map_4k` allocates missing intermediate tables on demand and sets `PTE_U`
  on the whole chain for user pages — the same code serves future processes.
- The kernel PML4 itself is untouched: `0x40000000` remains unmapped under
  it, which is what makes the demo meaningful.
- Limitation, by design: sharing `pd` means a user process could (in ring 3)
  read/write all of low memory — real privilege separation needs the
  higher-half move and `U`/`S` auditing, both deferred to later milestones.
- TLB flushing is implicit in the `mov cr3` (no PCID); that is also the
  correct invalidation primitive for the user-page teardown M2.4 will need.
