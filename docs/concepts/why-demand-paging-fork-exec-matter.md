# Why Demand Paging, COW Fork, and Exec Matter

Milestone 2.4 is where the kernel stops pre-allocating memory for programs
and starts *lying* to them — productively. Every mechanism here is an
instance of the same idea: the hardware page table is a contract the kernel
can bend, and the page-fault handler is where the lie is kept honest.

## Demand paging is why "memory" is a fiction programs can rely on

A program asks `sbrk` for a page and immediately writes to an address that
**does not exist**. It works anyway — the kernel catches the fault, conjures
a frame, and retries the instruction as if nothing happened. This one
mechanism underlies: lazy heap growth (`brk`/`sbrk`/`malloc` backing),
memory-mapped files (fault = read from disk), zero-fill-on-first-touch,
stack guard pages, and even swap. Without a resumable fault path, every
allocation is eager and every reservation is real memory — an OS that can't
defer work is an OS that can't overcommit, and overcommit is how real
systems get density.

It also flips a perspective worth internalizing: a page fault is not an
error. It is the *normal path* for memory that exists logically but not yet
physically. The same vector reports "you wrote through a bad pointer"
(process-kill signal) and "please allocate me a page" (expected behavior) —
the error code is the only thing telling them apart.

## COW is the cheapest concurrency primitive Unix ever invented

`fork`'s semantics demand that parent and child see the *same* memory
contents and *different* memory. Eager copy pays full price immediately for
pages that are usually never written — process creation would cost an
address space's worth of copying every time. COW pays nothing up front and
charges only the pages someone actually dirties. The mechanism is three
lines of idea — clone the tables, share the frames, drop `W` — and the rest
is the fault handler noticing which level said no.

The subtlety that survives into production thinking: **caching is the enemy
of state changes.** Clearing `W` in the parent's tables is not enough while
its TLB still remembers the page as writable — you must flush (`CR3`
reload). Every later milestone that edits live page tables hits the same
rule: the tables are the truth, the TLB is a cache, and caches need
invalidation (`invlpg` per page or a full `CR3` flush).

And the demo shows the one property that matters — **isolation without
eager cost**: the child wrote `'c'` and the parent still read `'D'`. Same
address, same contents at fork time, different physical frames after a
write. That is the entire memory-model contract `fork` makes.

## Exec completes the process lifecycle's shape

`fork` gives you a copy; `exec` gives you a *replacement*. Together they are
how every program you have ever launched started: `fork` a shell, `exec` the
binary. The implementation detail worth remembering is that exec had to
rewrite the *saved frame*, not the running state — because on this design,
a process is its `IrqFrame`; change the frame and the `iretq` delivers a
different program. The same trick generalizes to `wait`/`exit`/`kill`: once
context is data, the lifecycle is table surgery.

## What this milestone closed

Phase 2.4 finishes the x86-64 reproduction of the Mik-64 process model:
private address spaces, lazy memory, COW fork, exec, preemption, syscalls.
The remaining gap to the checkpoint ("x86-64 kernel reproduces Mik-64
behavior") is only integration polish — the concepts are all now running on
real hardware semantics: error codes, `cr2`, `invlpg`, per-level permission
bits, and a TLB that must be told when you change the truth.
