# ADR-015: Mik-FS Layout, fd Syscalls, and User-Pointer Validation

## Status

Accepted (Milestone 3.3).

## Context

The milestone needs: create/write/read files, list them, and exec a
program from disk — with data surviving a restart. Three design points
mattered:

1. **On-disk layout.** ext2/FAT carry real complexity (allocation
   bitmaps, indirect blocks, BPB parsing) for zero pedagogical return at
   this size.
2. **The syscall boundary.** Everything until now passed only register
   values — a filename means a user pointer, and a user pointer means
   validation, which the ABI had deliberately avoided.
3. **The fd model.** Per-process descriptor tables are the POSIX shape
   and make `fork` semantics honest (children inherit open files).

## Decision

**Mik-FS** (`fs.rs`) — flat, contiguous, bump-allocated:

```text
sector 256      superblock: magic "MIKFS001" + next_free_sector
sectors 257-258 directory: 32 entries x 32 B
                { name[24] NUL-padded, first_sector u32, byte_len u32 }
sectors 260+    file data — 8 contiguous sectors (4 KiB) per file
```

The whole 1 KiB directory is cached in memory; every mutation writes its
sector through immediately. If the superblock magic is absent the FS
formats itself and seeds `hello.txt` and `x` (prog_d's bytes — the
exec-from-disk target, so no build tooling has to write files).

**Syscalls 8-13**: `open(name,len,mode)->fd`, `close(fd)`,
`fread(fd)->byte`, `fwrite(fd,byte)`, `exec_file(name,len)`, `ls`.
Console syscalls 1/7 keep their old signatures — overloading them with
an fd argument would let a stale register silently redirect console I/O.
fds 0/1 are the console; 2-5 index a 4-slot table on the `Proc`, copied
on `fork` and inherited through `exec` (POSIX-ish).

**User-pointer validation** (`ustr_ok`): every page the buffer touches
must lie in the private user region `[0x40000000, 0x80000000)` and map
present+user via `find_pte`. Read-only use means W is not required —
fork children pass pointers into shared COW pages legitimately.

**`exit` now frees the slot** (`EMPTY` instead of a dead `DEAD` state) so
`run NAME` — which is `fork` + `exec_file` — can repeat instead of
exhausting the 4-slot table.

## Consequences

- Marked ceilings: space is never reclaimed (no free list — deletes and
  truncation leak sectors); 4 KiB fixed capacity; 32 files max; names
  <= 24 bytes; directory cached but never big enough to matter.
- `ustr_ok` is the first honest answer to "what does the kernel do with
  a user pointer" — it will be reused by any future pointer-carrying
  syscall.
- `ls` prints kernel-side rather than passing a directory buffer —
  simpler than a `readdir` ABI, sufficient for a 32-entry flat dir.
