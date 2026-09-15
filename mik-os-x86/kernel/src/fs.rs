//! Mik-FS: a flat, contiguous-allocation filesystem on the boot disk.
//!
//! Layout (sector = 512 B), placed past the 128 KiB kernel image:
//!
//! ```text
//! sector 256      superblock: magic "MIKFS001" + next_free_sector
//! sectors 257-258 directory: 32 entries x 32 B
//!                 { name[24] NUL-padded, first_sector u32, byte_len u32 }
//! sectors 260+    file data — 8 contiguous sectors (4 KiB) per file
//! ```
//!
//! Deliberate ceilings (marked for later milestones): space is bump-
//! allocated and never reclaimed; no subdirectories, permissions, or
//! timestamps; the whole 1 KiB directory is cached in memory and every
//! mutation writes its sector through immediately — correct and simple,
//! not fast.

use crate::ata;
use crate::serial;

const SB_LBA: u32 = 256;
const DIR_LBA: u32 = 257;
const DIR_SECTORS: u32 = 2;
const DATA_LBA: u32 = 260;
const FILE_SECTORS: u32 = 8;
const FILE_CAP: u32 = FILE_SECTORS * 512;
const MAX_FILES: usize = 32;
const ENT: usize = 32;
pub const NAME_MAX: usize = 24;
const MAGIC: &[u8; 8] = b"MIKFS001";

static mut PRESENT: bool = false;
static mut DIR: [u8; MAX_FILES * ENT] = [0; MAX_FILES * ENT];
static mut NEXT_FREE: u32 = 0;

pub fn present() -> bool {
    unsafe { *core::ptr::addr_of!(PRESENT) }
}

fn ent_len(dir: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(dir[i * ENT + 28..i * ENT + 32].try_into().unwrap())
}

fn ent_sector(dir: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(dir[i * ENT + 24..i * ENT + 28].try_into().unwrap())
}

/// Write the cached sector holding entry `i` back to disk.
unsafe fn flush_ent(i: usize) {
    let dir = &*core::ptr::addr_of!(DIR);
    let sec = i * ENT / 512;
    ata::write_sector(DIR_LBA + sec as u32, dir[sec * 512..].as_ptr());
}

unsafe fn flush_sb() {
    let mut sb = [0u8; 512];
    sb[..8].copy_from_slice(MAGIC);
    sb[8..12].copy_from_slice(&(*core::ptr::addr_of!(NEXT_FREE)).to_le_bytes());
    ata::write_sector(SB_LBA, sb.as_ptr());
}

/// Find a file by exact name. Returns its directory index.
pub unsafe fn find(name: &[u8]) -> Option<usize> {
    if !present() || name.is_empty() || name.len() > NAME_MAX {
        return None;
    }
    let dir = &*core::ptr::addr_of!(DIR);
    (0..MAX_FILES).find(|&i| {
        let e = &dir[i * ENT..i * ENT + ENT];
        e[0] != 0
            && e[..name.len()] == *name
            && (name.len() == NAME_MAX || e[name.len()] == 0)
    })
}

/// Create a file (contiguous slot bump-allocated) or None if the
/// directory/disk is full.
pub unsafe fn create(name: &[u8]) -> Option<usize> {
    if !present() || name.is_empty() || name.len() > NAME_MAX {
        return None;
    }
    let dir = &mut *core::ptr::addr_of_mut!(DIR);
    let i = (0..MAX_FILES).find(|&i| dir[i * ENT] == 0)?;
    let first = *core::ptr::addr_of!(NEXT_FREE);
    *core::ptr::addr_of_mut!(NEXT_FREE) = first + FILE_SECTORS;
    let e = &mut dir[i * ENT..i * ENT + ENT];
    e.fill(0);
    e[..name.len()].copy_from_slice(name);
    e[24..28].copy_from_slice(&first.to_le_bytes());
    flush_ent(i);
    flush_sb();
    Some(i)
}

/// Truncate a file to zero bytes (open-for-write semantics). The slot's
/// sectors are kept — Mik-FS never shrinks allocations.
pub unsafe fn truncate(i: usize) {
    let dir = &mut *core::ptr::addr_of_mut!(DIR);
    dir[i * ENT + 28..i * ENT + 32].copy_from_slice(&0u32.to_le_bytes());
    flush_ent(i);
}

/// Byte `pos` of file `i`, or None at/after EOF.
pub unsafe fn read_at(i: usize, pos: u32) -> Option<u8> {
    let dir = &*core::ptr::addr_of!(DIR);
    if pos >= ent_len(dir, i) {
        return None;
    }
    let mut sec = [0u8; 512];
    if !ata::read_sector(ent_sector(dir, i) + pos / 512, sec.as_mut_ptr()) {
        return None;
    }
    Some(sec[(pos % 512) as usize])
}

/// Write byte `pos` of file `i` (read-modify-write its sector). Extends
/// the recorded length on append. False past the 4 KiB capacity.
pub unsafe fn write_at(i: usize, pos: u32, b: u8) -> bool {
    if pos >= FILE_CAP {
        return false;
    }
    let dir = &mut *core::ptr::addr_of_mut!(DIR);
    let lba = ent_sector(dir, i) + pos / 512;
    let mut sec = [0u8; 512];
    if !ata::read_sector(lba, sec.as_mut_ptr()) {
        return false;
    }
    sec[(pos % 512) as usize] = b;
    if !ata::write_sector(lba, sec.as_ptr()) {
        return false;
    }
    if pos + 1 > ent_len(dir, i) {
        dir[i * ENT + 28..i * ENT + 32].copy_from_slice(&(pos + 1).to_le_bytes());
        flush_ent(i);
    }
    true
}

/// Read a whole file into `buf`; returns the byte count.
pub unsafe fn read_file(i: usize, buf: &mut [u8]) -> usize {
    let dir = &*core::ptr::addr_of!(DIR);
    let len = (ent_len(dir, i) as usize).min(buf.len());
    let mut done = 0;
    while done < len {
        let mut sec = [0u8; 512];
        if !ata::read_sector(ent_sector(dir, i) + (done / 512) as u32, sec.as_mut_ptr()) {
            break;
        }
        let n = (len - done).min(512);
        buf[done..done + n].copy_from_slice(&sec[..n]);
        done += n;
    }
    done
}

/// Iterate live files: `f(name, byte_len)`.
pub unsafe fn each_file(mut f: impl FnMut(&[u8], u32)) {
    if !present() {
        return;
    }
    let dir = &*core::ptr::addr_of!(DIR);
    for i in 0..MAX_FILES {
        let e = &dir[i * ENT..i * ENT + ENT];
        if e[0] == 0 {
            continue;
        }
        let nlen = e[..NAME_MAX].iter().position(|&b| b == 0).unwrap_or(NAME_MAX);
        f(&e[..nlen], ent_len(dir, i));
    }
}

unsafe fn seed(name: &[u8], content: &[u8]) {
    if let Some(i) = create(name) {
        for (p, &b) in content.iter().enumerate() {
            write_at(i, p as u32, b);
        }
    }
}

/// Mount Mik-FS: read the superblock and cache the directory. A missing
/// or wrong magic formats the FS area and seeds `hello.txt` plus `x`
/// (the exec-from-disk demo program). Returns the live file count.
pub unsafe fn init(seed_prog: &[u8]) -> u32 {
    if !ata::present() {
        return 0;
    }
    let mut sb = [0u8; 512];
    let fresh = !(ata::read_sector(SB_LBA, sb.as_mut_ptr()) && sb[..8] == MAGIC[..]);
    *core::ptr::addr_of_mut!(PRESENT) = true;
    if fresh {
        (*core::ptr::addr_of_mut!(DIR)).fill(0);
        *core::ptr::addr_of_mut!(NEXT_FREE) = DATA_LBA;
        flush_sb();
        for s in 0..DIR_SECTORS {
            ata::write_sector(DIR_LBA + s, [0u8; 512].as_ptr());
        }
        seed(b"hello.txt", b"Hello from Mik-FS\n");
        seed(b"x", seed_prog);
        serial::write_str("fs: formatted\n");
    } else {
        *core::ptr::addr_of_mut!(NEXT_FREE) =
            u32::from_le_bytes(sb[8..12].try_into().unwrap());
        let dir = &mut *core::ptr::addr_of_mut!(DIR);
        for s in 0..DIR_SECTORS {
            ata::read_sector(
                DIR_LBA + s,
                dir[s as usize * 512..].as_mut_ptr(),
            );
        }
    }
    let mut n = 0;
    each_file(|_, _| n += 1);
    n
}
