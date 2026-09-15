# Why File Systems Matter

An OS without storage forgets everything the moment it stops running.
Every milestone before this one produced state that died with the VM:
processes, page tables, the demo sequence. The filesystem is where a
machine starts to *remember* — and it changes what the OS is for.

## Persistence is the product

The milestone's acceptance test is the honest one: write a file, kill
QEMU, boot again, read it back. That one property — data outliving the
run — is what separates a computer from a calculator. It's also why the
driver detail that actually matters is FLUSH CACHE, not sector transfer:
a write that sits in a device cache is a promise, not a fact. Every real
storage stack is built around that distinction (fsync, write barriers,
journals all exist to control *when* a write becomes durable).

## Files are the universal namespace

Before Mik-FS, the kernel had exactly one kind of object you could name:
a process slot. Now there are named, byte-addressable, durable objects —
and the shell can `ls` them, `cat` them, `w` new ones into existence,
and `run` them. A directory of named programs is what turns "exec the
one hardcoded blob" into a real operating system: `run x` is the first
command that does something the kernel didn't know about at build time
only in the sense that the *file table*, not the code, decided what runs.

## The syscall boundary finally grows up

Everything before passed values in registers — safe because registers
can't point anywhere. `open(name, len)` passes a pointer, and a pointer
is a claim: "these addresses are mine, mapped, and readable." `ustr_ok`
is the first place the kernel *checks* that claim by walking the
caller's page tables. That pattern — validate every user-supplied
address against the caller's own mappings — is the difference between a
kernel and a rootkit delivery mechanism, and it's why this tiny check
matters more than the filesystem around it.

## Everything after this is a file problem

The remaining milestones lean on storage directly: networking stacks
want config and sockets-as-fds; a real init wants to exec its first
program *from disk*; SMP doesn't care, but every userspace tool it would
run does. `fork`+`exec_file` also closes the loop started in M2.4: the
process model now pairs with a place for programs to live — which is
the complete shape of a tiny UNIX.
