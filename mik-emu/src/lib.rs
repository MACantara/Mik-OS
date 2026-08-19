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
    pub mem: Vec<u8>,
    pub halted: bool,
    pub exit_code: u8,
}

impl Machine {
    pub fn new() -> Self {
        let mut m = Machine {
            regs: [0; 16],
            pc: LOAD_ADDR,
            epc: 0,
            mem: vec![0; RAM_SIZE as usize],
            halted: false,
            exit_code: 0,
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

    pub fn step<W: Write>(&mut self, out: &mut W) -> Result<(), String> {
        let pc = self.pc as usize;
        if pc + 8 > self.mem.len() {
            return Err(format!("pc out of bounds: {:#x}", self.pc));
        }
        if self.pc < LOAD_ADDR || self.pc >= RAM_SIZE || self.pc % 8 != 0 {
            return Err(format!("pc misaligned or in reserved memory: {:#x}", self.pc));
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

        // The address of the current instruction, used for branch targets.
        let base = self.pc;
        // Default: advance to the next instruction.
        self.pc += 8;

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
                let addr = self.regs[rs1].wrapping_add(imm as u64);
                self.regs[rd] = self.read8(addr)? as u64;
            }
            0x08 => {
                // LOAD64
                let addr = self.regs[rs1].wrapping_add(imm as u64);
                self.regs[rd] = self.read64(addr)?;
            }
            0x09 => {
                // STORE8
                let addr = self.regs[rs1].wrapping_add(imm as u64);
                self.write8(addr, self.regs[rs2] as u8, out)?;
            }
            0x0A => {
                // STORE64
                let addr = self.regs[rs1].wrapping_add(imm as u64);
                self.write64(addr, self.regs[rs2])?;
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
                // and jump through the trap vector at 0x2000.
                self.epc = self.pc;
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
