# Mik OS — Agent Notes

## Project Goal

Build a from-scratch operating system named Mik OS. The first phase uses a
custom virtual machine called Mik-64 and a Rust emulator to learn OS internals
without first fighting x86-64 hardware. The long-term target is a real x86-64
port under QEMU or Bochs.

See [`README.md`](README.md) for the public project overview and quick start.

## Stack

- **Host language:** Rust
- **Emulator:** `mik-emu` crate in the workspace
- **Kernel language for the MVP:** Hand-assembled using `mik_emu::encode` in the
  `mik-os` crate. The real Rust Mik OS will come with the x86-64 port.

## Build and Test Commands

```bash
# Build everything
cargo build

# Run tests
cargo test

# Build and boot Mik OS in one command (PowerShell)
.\run.ps1

# Manual steps
cargo run -p mik-os -- target/mik-64-kernel.bin
cargo run -p mik-emu -- target/mik-64-kernel.bin

# x86-64 kernel (Milestone 2.1): needs the bare-metal target and QEMU
rustup target add x86_64-unknown-none
cargo run -p mik-os-x86 -- qemu   # BIOS disk image: boot16 -> stage2 -> kernel
cargo run -p mik-os-x86 -- pvh    # QEMU -kernel direct-boot comparison path
cargo run -p mik-os-x86 -- image  # build target/mik-os-x86.img only
# `mik-os-x86` finds QEMU via $QEMU, then PATH, then C:\Program Files\qemu.
```

## Project Layout

```
docs/
  ideas/
    mik-os-vm-first.md      # Why and how we chose the VM-first approach
  specs/
    mik-64.md               # Complete Mik-64 machine specification
  concepts/
    x86-boot-and-idt.md     # BIOS boot chain, long-mode transition, IDT
    why-x86-boot-and-idt-matter.md
    x86-memory-management.md # E820, free-list allocator, paging, CR3
    why-x86-memory-matters.md
    x86-interrupts-and-scheduling.md # GDT/TSS, iret frame, PIC/PIT, sched
    why-interrupts-and-scheduling-matter.md
    x86-demand-paging-fork-exec.md   # pf error code, sbrk/brk, COW, exec
    why-demand-paging-fork-exec-matter.md
    x86-serial-shell.md              # UART poll, sys_read, ring-3 shell
    why-serial-shell-matters.md
    x86-device-drivers.md            # PCI, IRQ input, blocking read, VGA
    why-device-drivers-matter.md
  architecture.md           # System architecture overview
  decisions/
    ADR-001-vm-first.md     # Why we built a custom VM first
    ADR-002-flat-memory-and-hand-assembly.md
    ADR-003-bios-disk-boot.md
    ADR-004-minimal-idt.md
    ADR-005-e820-memory-map.md
    ADR-006-address-space-sharing.md
    ADR-007-syscall-and-context-switch.md
    ADR-008-pic-pit-timer.md
    ADR-009-demand-paging.md
    ADR-010-cow-fork-and-exec.md
    ADR-011-serial-shell.md
    ADR-012-blocking-input.md
    ADR-013-console-drivers.md
    ADR-014-ata-pio-block-device.md
    ADR-015-mikfs-and-fd-syscalls.md
README.md                   # Public project overview
mik-emu/
  src/lib.rs                # Emulator library
  src/main.rs               # CLI wrapper
  tests/                    # Instruction-level tests
mik-os/
  src/lib.rs                # Hand-assembled Mik-64 kernel
  src/main.rs               # Binary builder
  user/                     # init.s, prog1.s — assembled at build time
  tests/                    # Kernel integration tests (boot, paging, os.rs demo)
mik-asm/
  src/lib.rs                # Text-to-binary Mik-64 assembler
  src/main.rs               # mik-asm <in.s> <out.bin>
  tests/assembler.rs        # Assembler unit tests
mik-os-x86/
  src/lib.rs                # ELF parser, disk-image builder, QEMU launcher
  src/main.rs               # mik-os-x86 <build|image|qemu|pvh>
  kernel/src/               # no_std x86-64 kernel (boot16/stage2, mem, sched,
                            # seg, pic, idt, serial, input, kbd, vga, pci,
                            # ata, fs, isr.S, user.S)
  tests/                    # build_kernel, disk_image, boots_in_qemu
run.ps1                     # One-command build/run
tasks/
  plan.md                   # Historical per-milestone plan (see ~/.devin/plans)
  todo.md                   # Progress checklist
```

## Important Decisions

- Flat physical memory at boot; the kernel builds page tables and enables 4-level
  paging via `PTBR` and `PMODE`.
- Mik-64 uses fixed 64-bit instruction words, a simple load/store architecture,
  and memory-mapped serial I/O.
- Initial boot: flat binary loaded at `0x400000`, `x15` (SP) set to `0x8000000`,
  PC set to `0x400000`.
- The trap vector lives at `0x2000`.
- The page-fault vector lives at `0x2010`.
- The bump allocator keeps its `next_page` counter at `0x700000`.
- The `0x700000` page is reserved metadata the allocator never hands out:
  allocator state, kernel scratch (`0x700018`..`0x700098`), and the two-slot
  process table at `0x700100`.
- User programs run at VA `0x800000` (PD index 4, per-process PT4); the demand
  region `0x800000`..`0xA00000` is paged in on fault. Syscalls: 0 halt,
  1 `print_char`, 2 `fork`, 3 `exec`, 4 `yield`, 5 `exit`.
- The x86-64 kernel ELF carries three payloads: `.boot16` (512-byte boot
  sector at `0x7C00`, kernel length patched at `0x1F8`), `.stage2` (32-bit
  loader at `0x7E00`), and the kernel at `0x400000`. `mik-os-x86`'s image
  builder packs them into a 1 MiB disk image (sector 0 = boot sector,
  stage2 at `0x7E00`, kernel sections ≥ `0x400000`; the loader reads sectors
  1–255 in two batches; sectors 256+ are Mik-FS). stage2 reproduces the
  PVH handoff so `_start` is shared. A 256-entry IDT prints `EXnn` and
  halts for exceptions.
- The boot sector collects the E820 memory map into phys `0x5000` (magic
  `'MMAP'`, u32 count, 24-byte entries); `e820.rs` parses it, with a
  synthetic fallback for the PVH path. `mem.rs` free-lists usable frames
  (excluding <1 MiB and the kernel image `0x400000..__bss_end`), extends the
  `.bss` PD to a 1 GiB identity map, and builds per-space PML4s that share
  the kernel PD at PDPT[0] with private user pages at `0x40000000`+.
  Ordering matters: `extend_identity_map` must run before `mem::init`
  because free-list seeding writes into every frame. Ring-3 access needs
  `PTE_U` at every walk level — `up4[0]` carries it, the shared kernel PD
  link does not.
- The x86 scheduler (`sched.rs`) treats one `IrqFrame` (15 GPRs + the iret
  frame) as a whole process context: `isr_timer`/`isr_syscall` build it,
  Rust handlers return which frame to resume, `irq_tail` + `iretq` performs
  the switch including `CR3`. Syscalls are `int 0x80` (rax=1 write,
  2 exit, 3 yield, 4 fork, 5 exec, 6 sbrk, 7 read, 8 open, 9 close,
  10 fread, 11 fwrite, 12 exec_file, 13 ls; rdi/rsi/rdx=args) through a
  DPL-3 interrupt gate. `sys_read` is **blocking**: empty input marks the proc
  WAITING, rewinds `frame.rip` over the `int 0x80` (CD 80 = 2 bytes), and
  a device-ISR wake replays the syscall. `TSS.rsp0` points
  at the scheduled process's kernel stack. The PIC is remapped to vectors
  32–47 with IRQ0 (timer), IRQ1 (keyboard), IRQ4 (UART) unmasked on the
  master; the
  PIT runs ~100 Hz and is armed last — `sched::start` first drains the
  BIOS-latched tick while the scheduler is inactive so it cannot preempt
  the first user instruction. Device input lands in a shared 64-byte ring
  (`input.rs`); the timer also drains the UART each tick (delivery safety
  net for missed IRQs), and an `IN_IDLE` gate drops ticks taken inside the
  all-waiting `sti;hlt` idle loop.
- Page faults are resumable: `isr_pf` saves GPRs first (clobbering `rsi`
  before SAVE_REGS leaks the error code into the frame), passes the code as
  arg2, and slides the iret frame over the code slot. `pf_handler`
  demand-maps not-present faults in `[0x40002000, brk)` (`sys_sbrk` only
  moves `brk`) and COW-copies present+write faults on `P|U|!W` leaves in
  the private region `0x40000000..0x80000000` — `invlpg` after each fix;
  other faults print `EX0E` + `cr2`/`err`/`rip` and halt. `sys_fork`
  clones the user PD/PT chain sharing leaf frames read-only on both sides
  (no refcount) and copies the live frame with rax=0; `sys_exec` swaps in
  a fresh table running `prog_c` and rewrites the frame — old user tables
  leak (marked).
- The serial shell (`prog_sh` in `user.S`, spawned third; `NPROC`=4) prints
  `mik> `, buffers a line in user space (echo, Enter submits \r or \n,
  backspace erases), splits CMD [ARGS], and dispatches `ls`, `cat NAME`,
  `run NAME` (fork+exec_file), `mk NAME`, `w NAME TEXT`, `v`, `q`.
  `sys_write` mirrors to COM1 and the VGA text buffer at `0xB8000`
  (`vga.rs`); `pci::scan()` enumerates the bus at boot.
- Storage: `ata.rs` is a polled LBA28 PIO driver on the primary-bus master
  (FLUSH CACHE after every write — that's what makes files durable under
  QEMU). `fs.rs` is Mik-FS: superblock at sector 256 (`MIKFS001` magic +
  bump `next_free`), a 32-entry directory cached in memory and written
  through, and contiguous 4 KiB file slots from sector 260. Missing magic
  formats and seeds `hello.txt` + `x` (prog_d). File syscalls take the
  first user pointers — `ustr_ok` requires each page in the private user
  region mapped P|U. Per-process 4-slot fd tables (fds 0/1 = console) are
  copied on `fork`; `exit` marks the slot EMPTY so `run` repeats.
  `boots_in_qemu.rs` boots twice on one image: boot 1 drives
  `cat`/`run`/`w`, boot 2 `cat`s the written file to prove persistence.

See [`docs/decisions/`](docs/decisions/) for full ADRs.
