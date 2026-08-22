use std::io::Write;

const RAM_SIZE: u64 = 128 * 1024 * 1024;
const LOAD_ADDR: u64 = 0x400000;
const SERIAL_DATA: u64 = 0x1000;
const SERIAL_STATUS: u64 = 0x1001;
const IMM_MASK: u64 = (1u64 << 44) - 1;

/// Sign-extend a 44-bit two's-complement value to `i64`.
fn sext44(value: u64) -> i64 {
    let v = value & IMM_MASK;
    if v & (1u64 << 43) != 0 {
        (v | !IMM_MASK) as i64
    } else {
        v as i64
    }
}

/// Encode one Mik-64 instruction word.
pub fn encode(opcode: u8, rd: u8, rs1: u8, rs2: u8, imm: i64) -> u64 {
    let imm_44 = (imm as u64) & IMM_MASK;
    ((opcode as u64) << 56)
        | ((rd as u64) << 52)
        | ((rs1 as u64) << 48)
        | ((rs2 as u64) << 44)
        | imm_44
}

/// A Mik-64 virtual machine instance.
pub struct Machine {
    pub regs: [u64; 16],
    pub pc: u64,
    pub epc: u64,
    pub csrs: [u64; 256],
    tlb: [TlbEntry; TLB_SIZE],
    pub mem: Vec<u8>,
    pub halted: bool,
    pub exit_code: u8,
    pub user_mode: bool,
    previous_user_mode: bool,
}

// CSR numbers.
pub const CSR_PTBR: u64 = 0;
pub const CSR_PMODE: u64 = 1;

// PTE bits.
const PTE_P: u64 = 1 << 0; // Present
const PTE_W: u64 = 1 << 1; // Writable
const PTE_U: u64 = 1 << 2; // User (accessible from user mode)
const PTE_A: u64 = 1 << 3; // Accessed
const PTE_D: u64 = 1 << 4; // Dirty
const PTE_NX: u64 = 1 << 63; // No Execute
const PTE_PPN_MASK: u64 = ((1u64 << 36) - 1) << 12; // bits 12..47

/// The type of memory access being translated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Load,
    Store,
    Fetch,
}

/// A page fault with a fault code and the faulting virtual address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageFault {
    pub code: u64,
    pub va: u64,
}

/// A single TLB entry.
#[derive(Debug, Clone, Copy)]
struct TlbEntry {
    valid: bool,
    vpn: u64,       // virtual page number (va >> 12)
    ppn: u64,       // physical page number (pa >> 12)
    pte_addr: u64,  // physical address of the leaf PTE (for A/D updates)
    writable: bool, // PTE.W
    nx: bool,       // PTE.NX
    user: bool,     // PTE.U
}

const TLB_SIZE: usize = 16;

impl TlbEntry {
    const fn empty() -> Self {
        TlbEntry {
            valid: false,
            vpn: 0,
            ppn: 0,
            pte_addr: 0,
            writable: false,
            nx: false,
            user: false,
        }
    }
}

impl Machine {
    pub fn new() -> Self {
        let mut m = Machine {
            regs: [0; 16],
            pc: LOAD_ADDR,
            epc: 0,
            csrs: [0; 256],
            tlb: [TlbEntry::empty(); TLB_SIZE],
            mem: vec![0; RAM_SIZE as usize],
            halted: false,
            exit_code: 0,
            user_mode: false,
            previous_user_mode: false,
        };
        // x15 is the stack pointer by convention.
        m.regs[15] = RAM_SIZE;
        m
    }

    pub fn load_binary(&mut self, data: &[u8]) {
        let start = LOAD_ADDR as usize;
        let end = start.saturating_add(data.len()).min(self.mem.len());
        if end > start {
            self.mem[start..end].copy_from_slice(&data[..end - start]);
        }
    }

    fn read8(&self, addr: u64) -> Result<u8, String> {
        match addr {
            SERIAL_DATA => Ok(0),
            SERIAL_STATUS => Ok(0xFF),
            _ if addr < RAM_SIZE => Ok(self.mem[addr as usize]),
            _ => Err(format!("read8 out of bounds: {:#x}", addr)),
        }
    }

    fn read64(&self, addr: u64) -> Result<u64, String> {
        if addr + 8 > RAM_SIZE {
            return Err(format!("read64 out of bounds: {:#x}", addr));
        }
        let bytes = &self.mem[addr as usize..addr as usize + 8];
        Ok(u64::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn write8<W: Write>(&mut self, addr: u64, value: u8, out: &mut W) -> Result<(), String> {
        match addr {
            SERIAL_DATA => out.write_all(&[value]).map_err(|e| e.to_string()),
            SERIAL_STATUS => Ok(()),
            _ if addr < RAM_SIZE => {
                self.mem[addr as usize] = value;
                Ok(())
            }
            _ => Err(format!("write8 out of bounds: {:#x}", addr)),
        }
    }

    fn write64(&mut self, addr: u64, value: u64) -> Result<(), String> {
        if addr + 8 > RAM_SIZE {
            return Err(format!("write64 out of bounds: {:#x}", addr));
        }
        let bytes = value.to_le_bytes();
        self.mem[addr as usize..addr as usize + 8].copy_from_slice(&bytes);
        Ok(())
    }

    /// Flush the entire TLB.
    pub fn flush_tlb(&mut self) {
        for entry in self.tlb.iter_mut() {
            *entry = TlbEntry::empty();
        }
    }

    /// Translate a virtual address to a physical address.
    ///
    /// When paging is disabled (`PMODE = 0`), this is the identity function.
    /// When paging is enabled, it checks the TLB first, then walks the 4-level
    /// page table starting at `PTBR`. On success, returns the physical address.
    /// On failure, returns a `PageFault` with the fault code and faulting VA.
    pub fn translate(&mut self, va: u64, access: Access) -> Result<u64, PageFault> {
        if self.csrs[CSR_PMODE as usize] == 0 {
            return Ok(va);
        }

        // Check canonical address: bits 63..48 must be zero.
        if va >> 48 != 0 {
            return Err(PageFault { code: 4, va });
        }

        let vpn = va >> 12;
        let offset = va & 0xFFF;
        let tlb_index = (vpn as usize) & (TLB_SIZE - 1);

        // Check the TLB.
        let entry = &self.tlb[tlb_index];
        if entry.valid && entry.vpn == vpn {
            // TLB hit — check permissions from cached flags.
            if self.user_mode && !entry.user {
                return Err(PageFault { code: 5, va });
            }
            if access == Access::Store && !entry.writable {
                return Err(PageFault { code: 2, va });
            }
            if access == Access::Fetch && entry.nx {
                return Err(PageFault { code: 3, va });
            }
            // Update A/D bits in the leaf PTE on TLB hits.
            let pte = u64::from_le_bytes(
                self.mem[entry.pte_addr as usize..entry.pte_addr as usize + 8]
                    .try_into()
                    .unwrap(),
            );
            let mut new_pte = pte | PTE_A;
            if access == Access::Store {
                new_pte |= PTE_D;
            }
            if new_pte != pte {
                let bytes = new_pte.to_le_bytes();
                self.mem[entry.pte_addr as usize..entry.pte_addr as usize + 8]
                    .copy_from_slice(&bytes);
            }
            return Ok((entry.ppn << 12) | offset);
        }

        // TLB miss — walk the page table.
        let mut table = self.csrs[CSR_PTBR as usize] & !0xFFF; // page-align

        for level in (0..4u32).rev() {
            let shift = 12 + 9 * level;
            let index = ((va >> shift) & 0x1FF) as usize;
            let pte_addr = table + (index as u64) * 8;

            // Read the PTE from physical memory.
            if pte_addr + 8 > RAM_SIZE {
                return Err(PageFault { code: 1, va });
            }
            let pte = u64::from_le_bytes(
                self.mem[pte_addr as usize..pte_addr as usize + 8]
                    .try_into()
                    .unwrap(),
            );

            // Check present.
            if pte & PTE_P == 0 {
                return Err(PageFault { code: 1, va });
            }

            if level == 0 {
                // Leaf PTE — check permissions.
                if self.user_mode && pte & PTE_U == 0 {
                    return Err(PageFault { code: 5, va });
                }
                if access == Access::Store && pte & PTE_W == 0 {
                    return Err(PageFault { code: 2, va });
                }
                if access == Access::Fetch && pte & PTE_NX != 0 {
                    return Err(PageFault { code: 3, va });
                }

                // Set Accessed (and Dirty for stores) bits.
                let mut new_pte = pte | PTE_A;
                if access == Access::Store {
                    new_pte |= PTE_D;
                }
                if new_pte != pte {
                    let bytes = new_pte.to_le_bytes();
                    self.mem[pte_addr as usize..pte_addr as usize + 8]
                        .copy_from_slice(&bytes);
                }

                let ppn = (pte & PTE_PPN_MASK) >> 12;

                // Fill the TLB.
                self.tlb[tlb_index] = TlbEntry {
                    valid: true,
                    vpn,
                    ppn,
                    pte_addr,
                    writable: pte & PTE_W != 0,
                    nx: pte & PTE_NX != 0,
                    user: pte & PTE_U != 0,
                };

                return Ok((ppn << 12) | offset);
            } else {
                // Intermediate PTE — descend to next table.
                table = pte & PTE_PPN_MASK;
            }
        }

        // Unreachable: the loop always returns at level 0.
        unreachable!();
    }

    /// Deliver a page fault: save the faulting PC, set fault code and VA in
    /// registers, switch to supervisor mode, and jump to the page-fault
    /// handler at `mem64[0x2010]`.
    fn deliver_page_fault(&mut self, faulting_pc: u64, fault: PageFault) {
        self.epc = faulting_pc;
        self.previous_user_mode = self.user_mode;
        self.user_mode = false;
        self.regs[10] = fault.code;
        self.regs[11] = fault.va;
        // Read the handler address from physical memory (always physical).
        let handler = u64::from_le_bytes(
            self.mem[0x2010..0x2018].try_into().unwrap(),
        );
        self.pc = handler;
    }

    pub fn step<W: Write>(&mut self, out: &mut W) -> Result<(), String> {
        let base = self.pc;

        // Translate the PC for instruction fetch.
        let phys_pc = match self.translate(base, Access::Fetch) {
            Ok(pa) => pa,
            Err(fault) => {
                self.deliver_page_fault(base, fault);
                self.regs[0] = 0;
                return Ok(());
            }
        };

        let pc = phys_pc as usize;
        if pc + 8 > self.mem.len() {
            return Err(format!("pc out of bounds: {:#x}", phys_pc));
        }
        if phys_pc % 8 != 0 {
            return Err(format!("pc misaligned: {:#x}", phys_pc));
        }

        let word = u64::from_le_bytes(
            self.mem[pc..pc + 8]
                .try_into()
                .map_err(|_| "failed to read instruction".to_string())?,
        );

        let opcode = ((word >> 56) & 0xFF) as u8;
        let rd = ((word >> 52) & 0xF) as usize;
        let rs1 = ((word >> 48) & 0xF) as usize;
        let rs2 = ((word >> 44) & 0xF) as usize;
        let imm = sext44(word);

        // Default: advance to the next instruction.
        self.pc = base + 8;

        match opcode {
            0x00 => {
                // HALT
                self.halted = true;
                self.exit_code = 0;
            }
            0x01 => {
                // LI
                self.regs[rd] = imm as u64;
            }
            0x02 => {
                // ADD
                self.regs[rd] = self.regs[rs1].wrapping_add(self.regs[rs2]);
            }
            0x03 => {
                // ADDI
                self.regs[rd] = self.regs[rs1].wrapping_add(imm as u64);
            }
            0x04 => {
                // SUB
                self.regs[rd] = self.regs[rs1].wrapping_sub(self.regs[rs2]);
            }
            0x05 => {
                // AND
                self.regs[rd] = self.regs[rs1] & self.regs[rs2];
            }
            0x06 => {
                // OR
                self.regs[rd] = self.regs[rs1] | self.regs[rs2];
            }
            0x07 => {
                // LOAD8
                let va = self.regs[rs1].wrapping_add(imm as u64);
                let pa = match self.translate(va, Access::Load) {
                    Ok(p) => p,
                    Err(fault) => {
                        self.deliver_page_fault(base, fault);
                        self.regs[0] = 0;
                        return Ok(());
                    }
                };
                self.regs[rd] = self.read8(pa)? as u64;
            }
            0x08 => {
                // LOAD64
                let va = self.regs[rs1].wrapping_add(imm as u64);
                let pa = match self.translate(va, Access::Load) {
                    Ok(p) => p,
                    Err(fault) => {
                        self.deliver_page_fault(base, fault);
                        self.regs[0] = 0;
                        return Ok(());
                    }
                };
                self.regs[rd] = self.read64(pa)?;
            }
            0x09 => {
                // STORE8
                let va = self.regs[rs1].wrapping_add(imm as u64);
                let pa = match self.translate(va, Access::Store) {
                    Ok(p) => p,
                    Err(fault) => {
                        self.deliver_page_fault(base, fault);
                        self.regs[0] = 0;
                        return Ok(());
                    }
                };
                self.write8(pa, self.regs[rs2] as u8, out)?;
            }
            0x0A => {
                // STORE64
                let va = self.regs[rs1].wrapping_add(imm as u64);
                let pa = match self.translate(va, Access::Store) {
                    Ok(p) => p,
                    Err(fault) => {
                        self.deliver_page_fault(base, fault);
                        self.regs[0] = 0;
                        return Ok(());
                    }
                };
                self.write64(pa, self.regs[rs2])?;
            }
            0x0B => {
                // BEQ
                if self.regs[rs1] == self.regs[rs2] {
                    self.pc = base.wrapping_add((imm * 8) as u64);
                }
            }
            0x0C => {
                // BNE
                if self.regs[rs1] != self.regs[rs2] {
                    self.pc = base.wrapping_add((imm * 8) as u64);
                }
            }
            0x0D => {
                // JMP
                self.pc = base.wrapping_add((imm * 8) as u64);
            }
            0x0E => {
                // TRAP: save return address, set syscall number in x10,
                // switch to supervisor mode, and jump through the trap
                // vector at 0x2000.
                self.epc = self.pc;
                self.previous_user_mode = self.user_mode;
                self.user_mode = false;
                self.regs[10] = imm as u64;
                let vector = self.read64(0x2000)?;
                self.pc = vector;
            }
            0x0F => {
                // JMPR
                self.pc = self.regs[rs1];
            }
            0x10 => {
                // ERET
                self.pc = self.epc;
                self.user_mode = self.previous_user_mode;
            }
            0x14 => {
                // SRET: jump to regs[rs1] and enter user mode. This is the
                // supervisor's way of returning/entering a user process.
                self.pc = self.regs[rs1];
                self.previous_user_mode = self.user_mode;
                self.user_mode = true;
            }
            0x11 => {
                // RDCSR: rd = CSR[imm]
                let csr = (imm as u64) as usize & 0xFF;
                self.regs[rd] = self.csrs[csr];
            }
            0x12 => {
                // WRCSR: CSR[imm] = rs1
                let csr = (imm as u64) as usize & 0xFF;
                self.csrs[csr] = self.regs[rs1];
                // Flush TLB on PTBR or PMODE writes.
                if csr == CSR_PTBR as usize || csr == CSR_PMODE as usize {
                    self.flush_tlb();
                }
            }
            0x13 => {
                // SFENCE: flush the TLB.
                self.flush_tlb();
            }
            _ => {
                return Err(format!("illegal opcode: {:#x}", opcode));
            }
        }

        // x0 is hard-wired to zero.
        self.regs[0] = 0;
        Ok(())
    }
}

/// Run a flat binary on a fresh Mik-64 machine, writing serial output to `out`.
/// Returns the machine's exit code.
pub fn run<W: Write>(binary: &[u8], out: &mut W) -> Result<u8, String> {
    let mut machine = Machine::new();
    machine.load_binary(binary);
    while !machine.halted {
        machine.step(out)?;
    }
    Ok(machine.exit_code)
}
