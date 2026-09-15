# ADR-010: Copy-on-Write `fork` and In-Place `exec`

## Status

Accepted

## Date

2026-10-02

## Context

Milestone 2.4: "`fork()` copies the page table with COW mappings" and
"`exec()` replaces the address space and loads a new user program."

Mik-64's `fork` copied eagerly — the roadmap explicitly asks for COW on
x86-64. The interesting sub-decisions:

1. How the shared/read-only state is represented (refcounted shared pages
   vs. simplest-possible "everyone is read-only, first writer copies").
2. What the child inherits: registers, open state, and — specific to this
   design — the saved `IrqFrame` that is the process's execution context.
3. How `exec` swaps address spaces mid-syscall: the return path must come
   back under a different `CR3` than the one the trap arrived under.

## Decision

**Fork = deep table clone + shared read-only leaves, no refcount.**
`mem::clone_user_table` builds a fresh PML4/PDPT/PD/PT chain for the private
user region whose leaf PTEs point at the parent's physical frames with `W`
cleared — and clears `W` on the parent's leaves too. The kernel PD stays
shared (supervisor-only, never COW). A write fault on such a page (err bits
P+W, leaf `P|U|!W`, inside the private region) allocates a frame, copies the
page, writes the new `P|W|U` PTE into the *faulting* process's table, and
retries. There is no reference count: each side takes its private copy on
first write, and the shared frame is orphaned if both write — marked as a
deliberate simplification; the upgrade path is a per-frame refcount so the
last writer reuses the shared frame instead of copying.

The child's execution context is a **copy of the parent's live `IrqFrame`**
on the child's own kernel stack, with `rax=0`; the parent's `rax` becomes 1.
Both resume at the instruction after `int 0x80` — the textbook fork
semantics. After clearing the parent's `W` bits, its `CR3` is reloaded to
flush TLB entries that still carry write permission.

**Exec = fresh table + rewritten frame + immediate `switch_cr3`.** A new
user table maps the embedded `prog_c` image at `0x40000000` plus a fresh
stack; the current `IrqFrame` is rewritten in place (zero GPRs,
`rip`=user entry, `rsp`=new stack top, same RPL-3 selectors and `0x202`
rflags); `procs[cur].pml4` and `CR3` switch before returning — the `iretq`
that "returns from the syscall" is simultaneously the entry into the new
program. The old user frames and tables leak (marked; freeing needs a table
walker over the private PD chain, and `prog_c` is the only exec target —
also marked).

## Alternatives Considered

- **Eager-copy fork** (Mik-64's model): trivially correct, but the roadmap
  asks for COW and the real thing is barely harder once the resumable fault
  path exists — the COW branch is ~10 lines in `pf_handler`.
- **Refcounted shared pages**: correct reclamation, but needs a frame-metadata
  array and an owner question on the last write. Deferred — the educational
  point (sharing + isolation) is already proven by the demo.
- **`exec` via a new process slot**: would avoid the in-place frame rewrite,
  but then the "caller" would have to be reaped and PIDs get confusing —
  exec semantically keeps the same process.
- **Deferring `switch_cr3` to the next `schedule()`**: broken — the returned
  frame's `rip` must be valid under the active `CR3` *now*.

## Consequences

- The demo proves isolation end-to-end: the child writes `'c'` to a page the
  parent still reads as `'D'` — output order `ADcEDp` on serial.
- `fork`'s TLB flush and `exec`'s mid-syscall `CR3` swap are the two places
  where the x86 port is strictly more subtle than Mik-64 (which had no
  cached translations to invalidate and switched `PTBR` freely).
- `NPROC` grew to 3 — the fork child needs a free slot while A and B live.
- New syscalls 4/5/6 (`fork`/`exec`/`sbrk`) keep the `rax`-number/`rdi`-arg
  ABI; return values go through `frame.rax`.
