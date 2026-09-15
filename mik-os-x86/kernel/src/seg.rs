//! GDT with user segments + a TSS for ring-3 work.
//!
//! The boot-time `gdt64` has only kernel code/data. User mode needs two more
//! descriptors (DPL=3) and a Task State Segment: when an interrupt or
//! `int 0x80` fires in ring 3, the CPU loads `RSP` from `TSS.rsp0` — that is
//! how each process gets its own kernel stack.

/// 64-bit Task State Segment. Only `rsp0` and `iopb` matter to us: `rsp0` is
/// the ring-0 stack for privilege transitions, and `iopb = size` means no
/// I/O permission bitmap exists (all port I/O from ring 3 will GP-fault).
#[repr(C, packed)]
pub struct Tss {
    _reserved0: u32,
    pub rsp0: u64,
    _rsp12: [u64; 2],
    _reserved1: u64,
    _ist: [u64; 7],
    _reserved2: u64,
    _reserved3: u16,
    iopb: u16,
}

// Selectors: index*8 | RPL. Ring 3 needs RPL=3 on its CS/SS; the kernel
// selectors (0x08/0x10) and the TSS selector (0x28, index 5) are baked into
// the assembly that uses them.
pub const UDATA: u16 = 0x18 | 3; // -> 0x1B
pub const UCODE: u16 = 0x20 | 3; // -> 0x23

static mut TSS: Tss = unsafe { core::mem::zeroed() };
// null, kcode, kdata, udata(DPL3), ucode(DPL3), TSS low, TSS high.
static mut GDT: [u64; 7] = [
    0,
    0x00af_9a00_0000_ffff, // kcode: same as boot.S gdt64
    0x00af_9200_0000_ffff, // kdata
    0x00af_f200_0000_ffff, // udata: 0x92 | DPL3 (bits 45-46)
    0x00af_fa00_0000_ffff, // ucode: 0x9a | DPL3
    0,
    0,
];

extern "C" {
    fn seg_reload(); // isr.S: reload ds/es/ss, retfq to CS=0x08, ltr TSS_SEL
}

/// Install the new GDT + TSS. Must run before any ring-3 entry.
///
/// Safety: reloads all segment registers and the task register; call once
/// during kernel init.
pub unsafe fn init() {
    let tss = core::ptr::addr_of_mut!(TSS);
    (*tss).iopb = core::mem::size_of::<Tss>() as u16; // no IOPB present
    let base = tss as u64;
    let limit = (core::mem::size_of::<Tss>() - 1) as u64;
    // System descriptor (type 0x89 = available 64-bit TSS) spans two entries.
    let gdt = core::ptr::addr_of_mut!(GDT);
    (*gdt)[5] = (limit & 0xFFFF)
        | (base & 0xFF_FFFF) << 16
        | (0x89u64) << 40
        | (base >> 24 & 0xFF) << 56;
    (*gdt)[6] = base >> 32;

    #[repr(C, packed)]
    struct Gdtr {
        limit: u16,
        base: u64,
    }
    let gdtr = Gdtr { limit: (7 * 8 - 1) as u16, base: gdt as u64 };
    core::arch::asm!("lgdt [{}]", in(reg) &gdtr, options(readonly, nostack));
    seg_reload();
}

/// Point `TSS.rsp0` at the top of the kernel stack the CPU should switch to
/// when the *next scheduled* process is interrupted in ring 3.
pub unsafe fn set_rsp0(rsp: u64) {
    (*core::ptr::addr_of_mut!(TSS)).rsp0 = rsp;
}
