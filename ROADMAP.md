# Mik OS Roadmap

This roadmap maps the path from the current Mik-64 learning emulator to a real x86-64 operating system running under QEMU or Bochs. It is intentionally educational: each phase is designed to teach one major OS concept before the next one is added.

## Goal

Build a from-scratch operating system named Mik OS and deeply understand its internals. The approach is to first explore the ideas in a simplified, self-built virtual machine and emulator, then port the lessons to real x86-64 hardware.

## Current State

Mik OS currently runs on the Mik-64 virtual machine inside a Rust emulator. The hand-assembled kernel demonstrates the following working concepts:

- Boot, memory-mapped serial I/O, and a simple `HALT` / `print_char` syscall model.
- A physical free-list allocator (`alloc_page` / `free_page`) headed at `0x700008`, falling back to the bump `next_page` counter at `0x700000`.
- Four-level paging with 4 KiB pages, a 16-entry direct-mapped TLB, and CSR-style `PTBR` / `PMODE` controls.
- An identity-mapped kernel page table and a kernel page-fault handler that prints `F<fault_code>`.
- User/supervisor mode with `SRET`, `PTE_U`, and a user-mode system call round-trip.
- A programmable interval timer, `INT`/`IRET`, and interrupt masking while a trap or interrupt handler runs.
- Demand paging: not-present faults in the user region (`0x800000`..`0xA00000`) allocate a physical page, fill the PTE, and retry the faulting instruction.
- A two-slot process table, per-process page-table chains sharing the kernel identity map, and round-robin scheduling driven by timer ticks.
- `fork` (eager copy), `exec`, `yield`, and `exit` syscalls; user register state, `pc`, and `PTBR` are saved and restored per process.
- A tiny text assembler (`mik-asm`) that produces flat Mik-64 binaries; `init` and `prog1` in `mik-os/user/` are assembled at kernel-build time and run as user programs.
- End-to-end build and run via `cargo test` and `run.ps1`.

Phase 1 is complete: two user processes run under a timer-driven round-robin
scheduler with per-process page tables, demand paging, and `fork`/`exec`/
`yield`/`exit`, demonstrated by `mik-os/tests/os.rs`. Deliberate
simplifications remain: `fork` copies eagerly instead of using COW, and there
is no pseudo file system / `READ` syscall yet. Phase 2 has started: Milestone
2.1 is complete — a custom boot sector and stage2 loader boot a real BIOS
disk image through real mode, protected mode, and into long mode, where the
kernel installs a GDT, a minimal IDT, initial page tables, and prints a
serial banner. The PVH direct-boot path remains available for comparison.
Milestone 2.2 is also complete — an E820-fed free-list frame allocator, a
kernel-owned 1 GiB identity map, and `CR3` switching to a second address
space that runs a user page. Milestone 2.3 is complete as well — user-mode
processes with a GDT/TSS, a PIC-remapped PIT tick at 100 Hz, an `int 0x80`
syscall gate, and a frame-return round-robin scheduler that interleaves two
ring-3 programs (`ABaA` on serial). Milestone 2.4 completes the Mik-64
process model on real hardware: a resumable page-fault handler demand-maps
`sbrk`'d heap pages, `fork` shares the parent's user frames copy-on-write
(parent still reads `'D'` after the child writes `'c'`), and `exec` swaps
the live process into a fresh address space running an embedded image —
serial shows `ADcEDp` with `B`s interleaved by the timer. Phase 3 has
begun with Milestone 3.1: a non-blocking `sys_read` over the COM1 UART and
a ring-3 console shell (`mik> ` prompt, `v`/`q`/echo builtins) running as
an ordinary scheduled process. Milestone 3.2 added real device drivers:
a PCI config-space scan, IRQ1 keyboard + IRQ4 UART input feeding a
blocking `sys_read` (a `WAITING` process state that re-executes
`int 0x80` on wake), a VGA text console mirrored with COM1, and shell
line discipline. Milestone 3.3 added persistent storage: a polled ATA
PIO driver on the boot disk, the flat contiguous **Mik-FS** layout at
sector 256+, per-process fd tables behind new syscalls
(`open`/`close`/`fread`/`fwrite`/`exec_file`/`ls` — the first to carry
user pointers, validated by page-walk), a shell with `ls`/`cat`/`run`/
`w`/`mk`, and `run NAME` as `fork`+`exec_file`. Writes survive a QEMU
restart via FLUSH CACHE; the dual-boot test proves it.

## Phase 1: Mik-64 OS Core (Complete Learning Sandbox)

The first objective is to prove every major OS concept on the safe, inspectable Mik-64 emulator before real hardware complicates things.

### Milestone 1.1 — Memory Management Beyond Bump Allocation

**Status:** Complete — free-list allocator, per-process page tables (each
process gets a PML4/PDPT/PD whose PD[4] points at a private PT4 covering the
user region), and demand paging all work. COW is deliberately skipped:
`fork` copies the user page eagerly.

**Goal:** Move from a one-way bump allocator to a richer physical and virtual memory manager.

- Implement a free-list / buddy allocator for physical pages.
- Add page-table allocation helpers (`pt_alloc`, `pt_free`).
- Support separate per-process page tables (not just the kernel identity map).
- Add demand paging: the page-fault handler allocates a physical page, fills the PTE, and retries.
- Add copy-on-write (COW) pages for fork() support later.

**Acceptance criteria:**

- A kernel test allocates, frees, and re-allocates physical pages with no leaks.
- A user test can map and touch a new virtual page; the fault handler maps it on demand.
- `cargo test` still passes and `run.ps1` still prints the boot message.

### Milestone 1.2 — User-Mode Processes and System Calls

**Status:** Complete — a two-slot process table plus `fork`, `exec`, `yield`,
and `exit` work end-to-end (syscall numbers 2–5 alongside halt and
`print_char`).

**Goal:** Introduce the process abstraction, user mode, and a proper syscall interface.

- Add user/supervisor privilege levels to Mik-64 and PTE flags (`PTE_U`).
- Add a system call table beyond `0` and `1` (e.g. `exit`, `fork`, `exec`, `sbrk`, `read`, `write`).
- Implement a simple `fork()` using COW pages.
- Implement `exec()` by replacing the process page table and loading a small user binary.
- Add a process table and PIDs.

**Acceptance criteria:**

- A hand-assembled user program can call `print_char` and `exit`.
- `fork` creates a child process that continues from the same PC.
- `exec` replaces the address space and starts at a new entry point.
- An integration test verifies a parent/child output sequence.

### Milestone 1.3 — Interrupts, Timer, and Preemptive Scheduling

**Status:** Complete — the timer interrupt vector points at the scheduler,
which saves x1..x14, `pc` (via `CSR_EPC`), and `PTBR` into the current slot
and round-robins to the next live process. Interrupts are masked while any
handler runs and re-enabled by `ERET`/`IRET`/`SRET`.

**Goal:** Replace the cooperative `TRAP` model with true interrupts and a preemptive scheduler.

- Add a programmable timer device (e.g. APIC-like) that raises an interrupt after a number of steps.
- Add an interrupt controller and `INT` / `IRET` instructions.
- Implement round-robin scheduling in the kernel.
- Save and restore process context (registers, page table, `pc`).

**Acceptance criteria:**

- Two user processes alternate output under the timer.
- A process can yield with a `sys_yield` syscall.
- The scheduler correctly saves and restores `PTBR`/`PMODE` on context switch.

### Milestone 1.4 — Tiny Mik-64 User Programs and Assembler

**Status:** Complete — `mik-asm`-built `init` and `prog1` are appended to the
kernel image and run as user programs; `exec` loads an assembler-produced
image over the caller's user page. The pseudo file system / `READ` syscall is
deferred to a later milestone.

**Goal:** Stop hand-assembling and build the smallest possible user-space build chain.

- Write a tiny Mik-64 assembler in Rust (text to flat binary).
- Define an ELF-like or flat binary user program format.
- Write a few small user programs in this assembly (e.g. `init`, `shell`, `cat`).
- Add a `READ` syscall and a simple in-memory pseudo file system or pipe for I/O.

**Acceptance criteria:**

- `mik-asm <foo.s> <foo.bin>` works and the output runs under `mik-emu`.
- The kernel can `exec` a user program built from the assembler.
- A user program can print a string and exit.

### Checkpoint: Mik-64 is a Miniature OS

**Status:** Reached — `mik-os/tests/os.rs` demonstrates all of it.

- Multi-process scheduling works.
- Syscalls, page tables, demand paging, and user mode are exercised.
- The kernel is no longer hand-assembled byte-by-byte; the assembler produces it or user programs.
- All tests pass.

## Phase 2: Transition to x86-64 (QEMU/Bochs)

This phase is the educational bridge from the clean Mik-64 world to the real, quirky x86-64 world.

### Milestone 2.1 — x86-64 Bootloader and Long Mode

**Status:** Complete — a custom 512-byte boot sector loads stage2 plus the flat
kernel via INT 13h, enables A20, and enters protected mode; stage2 copies the
kernel to `0x400000`, zeroes `.bss`, and jumps to the same `_start` contract
PVH delivers. `boot.S` then builds the PML4/PDPT/PD identity map and enters
long mode. The kernel installs a 32-entry exception IDT (`isr.S`/`idt.rs`) and
demonstrates it with `int3` (`EX03` on serial). `qemu` boots the BIOS disk
image; `pvh` keeps the direct-boot path. Verified by
`mik-os-x86/tests/boots_in_qemu.rs` and `tests/disk_image.rs`.

**Goal:** Boot a real x86-64 kernel image under QEMU without any borrowed UEFI/GRUB code.

- Write a custom boot sector / stage1 loader that reads the kernel from a disk image or is embedded in a multiboot image.
- Switch from 16-bit real mode to 32-bit protected mode, then to 64-bit long mode.
- Set up a minimal GDT and IDT.
- Establish an initial page table for long mode.
- Print a character over serial (QEMU `-serial stdio`) as the first sign of life.

**Acceptance criteria:**

- `cargo run -p mik-os-x86 -- qemu` starts QEMU and prints a boot banner.
- The kernel is loaded above `0x400000` and begins executing in long mode.
- No hand-wavy BIOS calls remain in the boot path; the transition is fully self-contained.

### Milestone 2.2 — x86-64 Paging and Memory Management

**Status:** Complete — the boot sector collects the BIOS E820 memory map into
a fixed buffer at `0x5000` (the PVH path uses a documented synthetic
fallback); `mem.rs` builds a free-list frame allocator over the usable
regions (excluding <1 MiB and the kernel image); the identity map is extended
to 1 GiB from Rust by filling the `.bss` PD and reloading `CR3`; and a second
address space — a PML4 sharing the kernel PD with a private user PD — runs a
`PTE_U` page at `0x40000000`, proven by the `U` byte on serial under its own
CR3. Verified by `mik-os-x86/tests/boots_in_qemu.rs`.

**Goal:** Re-implement the Mik-64 memory concepts on real x86-64 page tables.

- Parse the memory map provided by the bootloader or BIOS/UEFI.
- Build a physical page allocator from available RAM.
- Implement the x86-64 version of the kernel identity mapping.
- Add `CR3` page table base switching for the first process.

**Acceptance criteria:**

- The kernel runs with 4-level paging enabled.
- A simple physical allocator can allocate and free 4 KiB frames.
- A user program can be mapped into a separate address space.

### Milestone 2.3 — x86-64 Interrupts, Syscalls, and Scheduling

**Status:** Complete — the GDT gained user code/data descriptors and a TSS
(`seg.rs`); the PIC is remapped to vectors 32–47 with only IRQ0 unmasked and
the PIT runs at ~100 Hz (`pic.rs`); the 256-entry IDT adds `isr_timer` and
`isr_syscall` stubs that save 15 GPRs and hand Rust a full `IrqFrame`
(`isr.S`, `idt.rs`); `int 0x80` (DPL-3 interrupt gate) carries `sys_write`,
`sys_exit`, and `sys_yield` on `rax`/`rdi`; `sched.rs` holds a two-slot
process table whose context switch is "return a different frame pointer" —
`irq_tail` + `iretq` restores `rip`, `cs`, `rflags`, `rsp`, `ss`, and the
CR3 switch swaps the address space. Boot drains the BIOS-latched PIT tick
before the first dispatch. Two user programs interleave `ABaA` on serial —
process B never yields, so every exit from B is a genuine timer preemption.
Verified by `mik-os-x86/tests/boots_in_qemu.rs` on both BIOS and PVH paths.

**Goal:** Port the Mik-64 process model to x86-64.

- Set up the IDT for hardware exceptions and a timer (PIT/HPET/LAPIC).
- Use `syscall`/`sysret` or `int 0x80` for system calls.
- Implement context switch (save/restore `rsp`, `rflags`, `cs`, `ss`, page table).
- Port the round-robin scheduler from Mik-64.

**Acceptance criteria:**

- Timer interrupt fires and the scheduler switches processes.
- A user program can `sys_write` to the QEMU serial port and `sys_exit`.
- Two user programs run concurrently and interleave output.

### Milestone 2.4 — Demand Paging and Fork on x86-64

**Status:** Complete — vector 14 got a dedicated `isr_pf` stub that preserves
all GPRs, lifts the CPU error code into the handler's second argument, and
slides the iret frame over the code slot so `pf_handler` shares the
`IrqFrame`/`irq_tail` return path. `sys_sbrk` (rax=6) lazily grows
`procs[cur].brk` over `0x40002000`; a not-present fault in `[base, brk)`
demand-maps a zeroed frame `P|W|U` with `invlpg` and retries. `sys_fork`
(rax=4) deep-clones the private user PD/PT chain sharing leaf frames
read-only on both sides (no refcount — each writer copies on first fault;
the parent TLB is flushed by a CR3 reload); a present+write fault on a
`P|U|!W` leaf copies the page for the faulting process only. `sys_exec`
(rax=5) builds a fresh user table around the embedded `prog_c` image,
rewrites the live `IrqFrame`, and `switch_cr3`s before returning — the
syscall's `iretq` is the new program's ring-3 entry. Serial shows `ADcEDp`
(with `B`s interleaved): demand write `'D'`, child COW write `'c'`, exec'd
`'E'`, parent still reads `'D'` (isolation), parent's own COW `'p'`.
Verified by `mik-os-x86/tests/boots_in_qemu.rs` on BIOS and PVH paths.

**Goal:** Bring over the richer memory features from Mik-64.

- Demand page faults allocate a physical frame and map it.
- `fork()` copies the page table with COW mappings.
- `exec()` replaces the address space and loads a new user program.

**Acceptance criteria:**

- A user program can `sbrk` and then access newly mapped memory.
- `fork` and `exec` integration tests pass.
- All prior `cargo test` equivalents still pass.

### Checkpoint: x86-64 Kernel Reproduces Mik-64 Behavior

- The x86-64 kernel can boot, schedule, handle syscalls, manage page tables, and run small user programs.
- The learning loop is closed: every concept proven in Mik-64 now works on real hardware (emulated).

## Phase 3: Real OS Features (Optional / Future)

Once the x86-64 kernel is solid, these features can be added in any order — but the milestone order below follows real dependencies: drivers teach interrupt-driven I/O and PCI, which the file system and network card both build on; SMP comes last because it forces a locking audit of everything before it.

### Milestone 3.1 — Serial Console Shell (Complete)

**Status:** Complete — `sys_read` (syscall 7) polls COM1's line status register and returns one byte in `rax` or `-1` when empty, and `prog_sh` runs in ring 3 as a third spawned process: it prints `mik> `, echoes unknown input, prints a version banner on `v`, and exits on `q`, yielding its slice whenever no input is pending. Deliberate simplifications: input is a polled byte stream (no IRQ4, no line buffering or line discipline), `q` leaves the shell dead permanently, and there is no way to launch programs — builtins only.

### Milestone 3.2 — Real Device Drivers (Keyboard, VGA, PCI)

**Status:** Complete — `pci.rs` enumerates all buses via config ports
`0xCF8`/`0xCFC` and prints the QEMU topology (i440FX, PIIX3, VGA, e1000);
`serial::enable_rx_irq` arms UART IRQ4 + 16550 FIFOs and `kbd.rs` decodes
set-1 scancodes on IRQ1 — both feed a shared input ring buffer. `sys_read`
now blocks: a `WAITING` process rewinds `rip` over its `int 0x80` and
re-executes the syscall when an ISR wakes it; `schedule()` idles in
`sti;hlt` (under an `IN_IDLE` tick gate) if everything is blocked, and the
timer also drains the UART as a delivery safety net. `vga.rs` writes the
80x25 text console at `0xB8000` — `sys_write` mirrors to COM1+VGA, so the
VM is interactive from keyboard to window. The shell does line discipline
in user space (echo, Enter submits, backspace erases). Simplifications:
extended (`E0`) scancodes mostly ignored, PCI finds but does not attach
drivers, no kernel-side canonical tty.

**Goal:** Replace the polled serial-only console with real interrupt-driven devices — the first hardware the kernel drives that it did not invent.

- Add a PCI bus scan (config-space enumeration via ports `0xCF8`/`0xCFC`) — needed again by both the storage driver in M3.3 and the NIC in M3.4.
- Upgrade serial input from polled to interrupt-driven: unmask UART IRQ4, add a small RX ring buffer, and make `sys_read` block (a `WAITING` process state woken by the ISR) instead of returning `-1`.
- Add an IRQ1 PS/2 keyboard driver: scancode set-1 decoding to ASCII, feeding the same input buffer the shell reads.
- Add VGA text-mode output (writes to `0xB8000`) so the console works without `-serial stdio`; keep COM1 as the diagnostic/debug console.
- Add a minimal line discipline (buffer until `\n`, backspace handling) so the shell reads commands instead of raw bytes.

**Acceptance criteria:**

- Typing on the QEMU window keyboard produces input; output appears in the VGA window without serial.
- `sys_read` sleeps the shell process instead of spinning (observable: B's tick rate is unaffected and the shell doesn't consume slices while idle).
- `pci_scan` lists the QEMU devices (host bridge, ISA bridge, VGA, virtio/ATA disk, NIC) on serial.

### Milestone 3.3 — File System

**Status:** Complete — polled ATA PIO driver on the boot disk
(`ata.rs`, LBA28 + FLUSH CACHE), the flat contiguous **Mik-FS** layout
at sector 256+ (superblock + 32-entry directory + fixed 4 KiB file
slots, `fs.rs`), per-process fd tables behind syscalls
`open`/`close`/`fread`/`fwrite`/`exec_file`/`ls` — the first to take
user pointers, validated by `ustr_ok` page-walk — and a shell with
`ls`/`cat NAME`/`run NAME`/`mk NAME`/`w NAME TEXT`. `run` is
`fork`+`exec_file` and `exit` now frees the slot, so it repeats.
Deliberate simplifications (marked in code): space is bump-allocated and
never reclaimed; 32 files x 4 KiB max; no directories/permissions; PIO
instead of IRQ/DMA; no virtio.

**Acceptance criteria:**

- A file written through the syscall API survives a QEMU restart (data is on the disk image, not in RAM). ✅ — the test boots twice on one image and `cat`s the file written in boot 1.
- `ls` in the shell lists files; `cat` prints one; a program stored in Mik-FS runs in ring 3 via `exec`. ✅ — seeded `x` prints `X` via `run x`.
- All existing tests still pass (the FS must not disturb the process demo). ✅ — `ADcEDp` asserted in the same test.

### Milestone 3.4 — Networking

**Status:** Not started.

**Goal:** Send and receive real Ethernet frames through a virtual NIC — the smallest honest network stack.

- Add a NIC driver over the M3.2 PCI scan: QEMU's default e1000 (`0x8086:0x100E`) or virtio-net; TX/RX descriptor rings, IRQ11 interrupt delivery.
- Implement the minimum protocol stack: Ethernet framing → ARP (answer "who has <ip>") → IPv4 (header parse/build, checksum) → UDP (sockets as port mailboxes).
- Give processes `sys_sendto`/`sys_recvfrom` (or a net-only variant) and add shell commands `ping` (ICMP echo) and a UDP echo/print test.

**Acceptance criteria:**

- QEMU user-mode networking (`-netdev user`) shows the VM answering ARP and ICMP ping from the host-side gateway.
- A UDP packet sent from the host (e.g. `nc -u`) prints in the VM; a packet sent by the VM arrives on the host.

### Milestone 3.5 — Multi-Core / SMP

**Status:** Not started.

**Goal:** Run the scheduler on more than one CPU — the milestone that turns every "there is one current process" assumption into a real locking question.

- Parse the ACPI/MP tables (or QEMU's `-smp` topology) to discover APs; add a per-CPU data region (stack, TSS, current-process pointer).
- Implement the LAPIC + IOAPIC path (replace PIC routing; LAPIC timer per CPU) and the INIT–SIPI–SIPI AP startup dance with a real-mode trampoline page.
- Audit shared state: the process table, frame allocator, and console need spinlocks; the scheduler becomes a shared run queue (single queue is the honest minimum).
- Add TLB shootdown IPIs for when one CPU unmaps a page another CPU might have cached (first needed by COW/`exec` under SMP).

**Acceptance criteria:**

- `-smp 2` (or more) boots, both CPUs reach the scheduler, and serial shows interleaved output from processes running on different cores.
- The COW/`exec` demo still passes under two CPUs — no stale-TLB corruption.
- `-smp 1` still works identically (SMP is additive, not a rewrite).

## Non-Goals

These are deliberately out of scope to keep the project focused on understanding the core:

- Matching the full x86-64 instruction set or boot protocol in the Mik-64 phase.
- Production-grade performance, security hardening, or compatibility.
- A real GUI before the console and shell are solid.
- Real hardware boot in the first 6 months (QEMU/Bochs is the target).

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| The x86-64 long-mode boot dance is fiddly | High | Build it incrementally in QEMU; verify each mode transition with serial output before the next. |
| Real page tables and the TLB behave differently than Mik-64's | High | Write small focused tests in the x86-64 kernel before enabling user mode. |
| Multi-process context switch bugs are subtle | High | Keep the first scheduler non-preemptive until context save/restore is solid. |
| The hand-assembly bottleneck becomes painful | Medium | Prioritize the tiny assembler as soon as user programs are needed. |
| Scope creep into file systems / drivers | Medium | Phase 3 stays optional; do one milestone at a time and keep each shippable on its own. |

## How to Use This Roadmap

- Each milestone is a candidate for a focused implementation sprint.
- Before starting a milestone, write a spec and a detailed plan in `tasks/plan.md`.
- Update this roadmap when a milestone is complete or the long-term goal changes.
- The [Architecture](docs/architecture.md) and [Spec](docs/specs/mik-64.md) documents describe the current implementation and should be kept in sync with the roadmap.
