# Mik OS: VM-First Learning Direction

## Problem Statement

How might we design a from-scratch x86-64 operating system called Mik OS that gives me a deep understanding of OS internals, while sidestepping the pain of real hardware support by first building a simplified but realistic virtual machine and emulator?

## Recommended Direction

Build Mik OS in two acts. In **Act I**, design a minimal but x86-64-like virtual machine (working name **Mik-64**) and a small Rust emulator that can run it. The machine should have: a 64-bit register set, a simple instruction set, a **flat physical memory model (paging introduced later)**, a simple boot protocol, a minimal interrupt / syscall model, and a serial / terminal I/O device. This is the learning sandbox.

In **Act II**, once the core OS concepts (bootloader, memory management, process model, syscalls, scheduling) are working in this clean world, port Mik OS to real x86-64 under QEMU or Bochs. The fantasy machine is intentionally close to x86-64 so the port is an education in *legacy and quirks*, not a total rewrite.

Writing the emulator yourself is the key learning amplifier. It forces you to answer: what does an instruction actually *do*? What does it mean for a device to raise an interrupt? What is a syscall at the hardware boundary? These questions are usually hidden inside QEMU.

## Decisions and Directions

### 1. Start with a flat physical memory model

**Decision:** The Mik-64 VM will use a flat physical memory model for the MVP. There are no page tables, no virtual-to-physical translation, and no per-process address spaces in the first version.

**Flat memory vs. paging:**

| Flat physical memory | Paging / virtual memory |
|---|---|
| The address the CPU sends to memory is the physical byte location. `0x1000` is the byte at physical location `0x1000`. | Every address is translated through page tables before it reaches physical memory. |
| Simple to emulate, simple to reason about, easy to inspect with a hex dump. | Enables memory protection, per-process address spaces, demand paging, and shared memory. |
| No memory protection between kernel and user; one bug can corrupt everything. | Adds a lot of x86-64 complexity: page tables, page table entries, TLB, page faults, supervisor/user page flags. |
| Good for learning the *order* of kernel operations before the *isolation* of kernel operations. | Essential for a real multi-user OS, but it can obscure what the kernel is actually doing while you are still learning. |

**Why this direction:** Paging is one of the biggest "why is x86 like this" topics. By starting flat, you build a working kernel, scheduler, and syscall path first. When you later add paging, you will understand exactly what problem it solves and why the hardware has the quirks it does.

### 2. Suggested next directions

- **Emulator interpreter style:** Use a simple `match` on each instruction. This is fast enough for the MVP and is the easiest to debug. Do not optimize with a function table or JIT until the emulator is the bottleneck.
- **Boot protocol:** Load the flat kernel binary at a fixed address (for example, `0x400000`), set the stack pointer to the top of a known RAM region, set the program counter to the load address, and go. Document the exact initial register state in the Mik-64 spec.
- **User test program:** Start with a hand-assembled test binary. This keeps the build chain tiny and forces you to understand the exact encoding the emulator expects. Once the syscall path is proven, add a tiny Rust `no_std` program compiled for the Mik-64 target.

## Key Assumptions to Validate

- [ ] A simplified VM can be built in a few weeks without becoming its own project.
- [ ] The VM can be kept close enough to x86-64 that ~80% of the OS code transfers.
- [ ] Writing the emulator in Rust is a good forcing function, not a distraction.
- [ ] A serial-console-only OS is enough to prove every core OS concept.

## MVP Scope

In:
- A Rust-based emulator for the Mik-64 machine
- A toy machine spec (registers, instruction set, memory, boot, I/O, interrupts)
- A Mik OS kernel that: boots, sets up a simple memory manager, handles a basic interrupt/syscall, and prints to the serial console
- A build/run script that launches the emulator with Mik OS

Out (for now):
- Real x86-64 port
- File system
- Multi-process scheduling
- GUI
- User-mode programs (beyond a tiny test program)

## Not Doing (and Why)

- **Real hardware boot in the first 6 months** — it violates the hardware support constraint and adds thousands of lines before the OS concept is clear.
- **A custom file system** — block devices and persistence are huge; use in-memory pseudo-files or no FS until the core runs.
- **A graphical interface** — terminal I/O is enough to test the kernel and is portable.
- **User-space shell/compiler in the MVP** — focus on the kernel first; the "user program" can be a hand-assembled test binary.

## Open Questions

- What is the exact boot protocol? (Where does the emulator load the kernel? What is the initial register state?)
- Should the user-space test program be hand-assembled or a tiny Rust `no_std` binary?
