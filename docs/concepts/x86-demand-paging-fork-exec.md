# x86-64: Demand Paging, Copy-on-Write Fork, and Exec

Milestone 2.4 turns the page-fault exception from a diagnostic (`EX0E` and
halt) into a mechanism: lazily materializing heap pages, sharing pages
between parent and child until one of them writes, and replacing a running
program's whole address space mid-syscall.

## The page-fault contract

`#PF` is unusual among exceptions: it carries a CPU-pushed **error code**
(between the iret frame and the stub's saved registers) and reports the
faulting address in **`cr2`**. The error code's low bits classify the fault:

| bit | meaning |
|-----|---------|
| 0 (P) | 0 = page not present, 1 = present but protection violated |
| 1 (W) | 1 = the access was a write |
| 2 (U) | 1 = the fault happened in ring 3 |

`isr_pf` normalizes the stack into the shared `IrqFrame` shape: save all 15
GPRs *first* (writing the error code into `rsi` beforehand corrupts the
frame's `rsi` — the retry then inherits the code as a pointer, a bug that
produced a `cr2=6 err=7` mystery fault), read the code at `[rsp+120]` into
`rsi` as the handler's second argument, and slide the five iret words down
over the code slot. From there, `pf_handler` is an ordinary Rust function
that returns a frame — returning the same frame retries the faulting
instruction, which is the whole trick behind both demand paging and COW.

## Demand paging: `sbrk` moves a number, the fault does the work

`sys_sbrk(n)` adds `n` (page-rounded) to `procs[cur].brk` and returns the
old break — no page is mapped. The first touch of an address in
`[USER_DATA_VA, brk)` faults not-present; the handler allocates a zeroed
frame, maps it `P|W|U` in the *current* process's table, issues `invlpg` on
the faulting page (a cached not-present translation can linger), and
returns. The retry sees a present writable page. Nothing is allocated for
memory that is reserved but never touched — that is the entire point.

## COW fork: share now, pay on write

`fork` deep-clones the private user page-table chain (new PD/PT frames) but
points the child's leaf PTEs at the **same physical frames**, with `W`
cleared — on both sides. Reads never fault; either process's first write
does (present + write, `P|U|!W` leaf), and the fault handler copies the
page, rewrites only the faulting process's PTE, and retries. Writes are
isolated; reads stay shared; nobody pays for copies they never make. The
parent's `CR3` is reloaded after the `W`-clear because its TLB still caches
writable translations — the one place x86 makes you manage a cache Mik-64's
tiny TLB let you ignore.

Child context = a byte-copy of the parent's `IrqFrame` on the child's kernel
stack, `rax=0` vs parent's `rax=1` — `fork` returns twice, once in each
process, with different values.

## Exec: a syscall that never returns to the same program

`exec` builds a brand-new user table (embedded `prog_c` image + fresh stack
+ reset `brk`), swaps `procs[cur].pml4`, rewrites the live `IrqFrame` in
place — fresh `rip`/`rsp`, zeroed GPRs, unchanged RPL-3 selectors — and
calls `switch_cr3` *inside* the syscall handler. The `iretq` on the way out
is simultaneously the entry into the new program; there is no "return" in
the usual sense. The old user mappings leak deliberately (documented;
freeing them needs a table walker).

## The demo, end to end

```
prog A:  'A'; sbrk(4096); [heap]='D'; print 'D'      <- demand fault -> map
         fork; parent yields
child:   [heap]='c'; print 'c'                       <- COW fault -> copy
         exec -> prog C: print 'E'; exit
parent:  print [heap] -> 'D'   (child's write stayed private — the proof)
         [heap]='p'; print 'p'; exit                 <- parent's own COW
prog B:  'B'; spin                                   <- timer preempts it
```

Serial: `ADcEDp` with `B`s interleaved wherever the 100 Hz tick lands.
