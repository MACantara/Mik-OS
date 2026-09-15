//! E820 memory-map parsing. The boot sector collects the map via INT 15h in
//! real mode into a fixed physical buffer — BIOS services vanish once we enter
//! protected mode, so the map has to be captured before `CR0.PE`.

/// Physical address of the map buffer written by `boot16.S`.
const MAP_ADDR: usize = 0x5000;
const MAGIC: u32 = 0x50414D4D; // 'MMAP'

/// One INT 15h E820 entry: 24 bytes, little-endian.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct E820Entry {
    pub base: u64,
    pub len: u64,
    pub typ: u32,
    pub acpi: u32,
}

impl E820Entry {
    /// E820 type 1 = usable RAM.
    pub fn usable(&self) -> bool {
        self.typ == 1 && self.len > 0
    }
}

extern "C" {
    static __bss_end: u8;
}

fn kernel_end() -> u64 {
    let end = unsafe { &__bss_end as *const u8 as u64 };
    (end + 0xFFF) & !0xFFF
}

/// Fallback map for the PVH path, where no BIOS ran. QEMU defaults to
/// 128 MiB; we trust the conventional layout: 1–4 MiB is RAM below the kernel,
/// everything from the kernel's `.bss` end to 128 MiB is free.
fn fallback_map(buf: &mut [E820Entry; 32]) -> usize {
    buf[0] = E820Entry { base: 0x10_0000, len: 0x400000 - 0x10_0000, typ: 1, acpi: 0 };
    buf[1] = E820Entry {
        base: kernel_end(),
        len: 0x800_0000 - kernel_end(),
        typ: 1,
        acpi: 0,
    };
    2
}

/// Read the memory map into `buf`, returning the number of valid entries.
/// Uses the BIOS-collected map when its magic is present, else the fallback.
pub fn read_map(buf: &mut [E820Entry; 32]) -> usize {
    let raw = MAP_ADDR as *const u8;
    if unsafe { (raw as *const u32).read() } != MAGIC {
        return fallback_map(buf);
    }
    let count = unsafe { (raw.add(4) as *const u32).read() as usize }.min(32);
    for i in 0..count {
        buf[i] = unsafe { (raw.add(8 + i * 24) as *const E820Entry).read() };
    }
    count
}
