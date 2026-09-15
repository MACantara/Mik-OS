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
  architecture.md           # System architecture overview
  decisions/
    ADR-001-vm-first.md     # Why we built a custom VM first
    ADR-002-flat-memory-and-hand-assembly.md
    ADR-003-bios-disk-boot.md
    ADR-004-minimal-idt.md
    ADR-005-e820-memory-map.md
    ADR-006-address-space-sharing.md
README.md                   # Public project overview
mik-emu/
  src/lib.rs                # Emulator library
  src/main.rs               # CLI wrapper
  tests/                    # Integration tests
mik-os/
  src/lib.rs                # Hand-assembled Mik-64 kernel
  src/main.rs               # Binary builder
mik-asm/
  src/lib.rs                # Text-to-binary Mik-64 assembler
  src/main.rs               # mik-asm <in.s> <out.bin>
  tests/kernel_boot.rs      # Boot output test
  tests/kernel_paging.rs    # Paging enablement test
  tests/pagefault.rs        # Kernel page-fault handler test
run.ps1                     # One-command build/run
tasks/
  plan.md                   # Implementation plan with acceptance criteria
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
  builder packs them into a 64 KiB disk image; stage2 reproduces the PVH
  handoff so `_start` is shared. A 32-entry exception IDT prints `EXnn` and
  halts; `kmain` proves it with `int3` (runs last — it never returns).
- The boot sector collects the E820 memory map into phys `0x5000` (magic
  `'MMAP'`, u32 count, 24-byte entries); `e820.rs` parses it, with a
  synthetic fallback for the PVH path. `mem.rs` free-lists usable frames
  (excluding <1 MiB and the kernel image `0x400000..__bss_end`), extends the
  `.bss` PD to a 1 GiB identity map, and builds per-space PML4s that share
  the kernel PD at PDPT[0] with private user pages at `0x40000000`+.
  Ordering matters: `extend_identity_map` must run before `mem::init`
  because free-list seeding writes into every frame.

See [`docs/decisions/`](docs/decisions/) for full ADRs.
