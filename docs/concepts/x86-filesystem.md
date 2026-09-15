# x86-64: Block Devices, Mik-FS, and the File Descriptor ABI

Milestone 3.3 added the last piece of a usable system: persistent
storage. A shell command like `cat hello.txt` now crosses every layer the
project has built — this doc walks the stack top to bottom.

## The block device: ATA PIO

QEMU's default `-drive` attaches the image to the PIIX3 IDE controller
(the `class 0101` entry from the M3.2 PCI scan), which answers on the
fixed legacy ports — no PCI setup needed:

| Port    | Role                                        |
|---------|---------------------------------------------|
| 0x1F0   | data (16-bit)                               |
| 0x1F2   | sector count                                |
| 0x1F3-5 | LBA low/mid/high                            |
| 0x1F6   | drive select + LBA bits 24-27 + mode        |
| 0x1F7   | status (read) / command (write)             |
| 0x3F6   | alternate status                            |

A sector read (`ata::read_sector`) is: wait for `BSY` to clear, write
`0xE0|lba_hi` to the drive-select port plus count/LBA registers, issue
command `0x20` (READ SECTORS), wait for `DRQ`, then `rep insw` pulls 256
words into a buffer. A write mirrors it with `0x30`/`rep outsw` and ends
with `0xE7` (FLUSH CACHE) — the command that forces the data all the way
to the host image file. Everything is polled: no IRQ14, no DMA. On a
single CPU whose callers already run with interrupts off, polling is not
a shortcut — it's the right size.

## Mik-FS: the simplest honest layout

```text
sector 256      superblock: "MIKFS001" + next_free_sector
sectors 257-258 directory: 32 fixed entries x 32 B
sectors 260+    file data: 8 contiguous sectors (4 KiB) each
```

A file is a name, a start sector, and a length. Allocation is a bump
pointer in the superblock; reads/writes are sector read-modify-write.
The directory (1 KiB) is fully cached in memory and written through on
every change — with 32 entries, "cache coherence" is one `write_sector`
call. Boot does a mount: good magic loads the directory, bad magic
formats and seeds two files.

What Mik-FS deliberately omits (marked in code): free-space tracking
(deleted/truncated space leaks), directories, permissions, timestamps,
journaling. Each is a real topic; none is needed to teach what a file
*is*.

## The fd ABI — first pointers across the boundary

Until now every syscall passed only register values. `open` needs a
*string* — a user pointer — which forces the question every real kernel
answers: how do you know a userspace address is safe to dereference?

`ustr_ok` answers it with the machinery that already exists: walk the
caller's page tables with `find_pte` for every page the buffer touches,
requiring it to lie in the private user region and be mapped
present+user. The syscalls only *read* user memory, so `PTE_W` isn't
required — a fork child's shared COW page is a valid name buffer.

```text
sys_open(name, len, mode) -> fd | -1     8   mode 0=read 1=write(create/truncate)
sys_close(fd) -> 0 | -1                  9
sys_fread(fd) -> byte | -1               10
sys_fwrite(fd, byte) -> 0 | -1           11
sys_exec_file(name, len)                 12  like exec, image from Mik-FS
sys_ls                                   13  kernel prints the directory
```

fds live in a 4-slot table on each `Proc`: 0/1 are the console, 2-5 are
files. `fork` copies the table (the child inherits the parent's open
files, cursors now independent); `exec` keeps it. `exit` marks the
process slot `EMPTY` outright — slot reuse is what makes `run` a
repeatable command rather than a one-shot.

## `run x`: fork + exec_file

The shell can't exec directly — exec replaces the caller. So `run`
forks: the parent yields, the child calls `exec_file("x")`, which loads
the file's bytes into a fresh address space exactly like the embedded
`exec` path (`exec_image`). The child prints `X` and exits; its slot
returns to `EMPTY`; the parent re-prompts. That is the real UNIX
spawn primitive — `fork`+`exec` — built from parts the kernel already
had.
