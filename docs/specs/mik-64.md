# Mik-64 Machine Specification

This is the specification for the **Mik-64** virtual machine. It is a 64-bit
RISC-like machine kept intentionally simple so that a kernel can be written and
understood without first fighting the x86-64 legacy. In a later phase, Mik OS
will be ported to real x86-64.

## 1. Overview

- 64-bit general-purpose registers
- 64-bit physical address space (flat, no paging in the MVP)
- Fixed 64-bit instruction words
- Simple load/store architecture
- Memory-mapped serial I/O
- Minimal trap model (reserved for later kernel use)

## 2. Data types and widths

| Type | Width | Notes |
|------|-------|-------|
| Byte | 8 bits | |
| Halfword | 16 bits | Not used in the MVP instruction set |
| Word | 32 bits | |
| Doubleword | 64 bits | Natural register and bus width |

All multi-byte values are stored **little-endian** in memory.

## 3. Registers

### 3.1 General-purpose registers

There are 16 general-purpose integer registers, each 64 bits wide:

| Number | Name | Role |
|--------|------|------|
| 0 | `x0` | **Hard-wired zero.** Reads always return 0; writes are ignored. |
| 1-14 | `x1` .. `x14` | General purpose |
| 15 | `x15` | General purpose; by convention the **stack pointer** (`sp`) |

### 3.2 Special registers

| Name | Width | Role |
|------|-------|------|
| `pc` | 64 bits | Program counter |

There is no privileged status register, no flags register, and no page table
base register in the MVP.

## 4. Memory model

Mik-64 uses a **flat physical memory model** in the MVP. Every address is a
physical byte address. There are no page tables, no virtual-to-physical
translation, and no memory protection.

The default machine has 128 MiB of RAM:

```
0x0000_0000 .. 0x0000_0FFF  : reserved zero page
0x0000_1000                 : serial data port
0x0000_1001                 : serial status port
0x0000_1002 .. 0x0000_1FFF  : reserved
0x0000_2000                 : trap vector (holds the 64-bit handler address)
0x0000_2008 .. 0x003F_FFFF  : reserved
0x0040_0000 .. 0x7FFF_FFFF  : general RAM (126 MiB)
0x8000_0000                 : initial stack pointer (top of RAM)
```

The zero page, serial ports, and the reserved region below `0x400000` are
considered **MMIO / reserved** and do not contain normal RAM.

## 5. Instruction format

All instructions are exactly 64 bits (one doubleword). The encoding is:

```
| 63..56 | 55..52 | 51..48 | 47..44 | 43..0 |
| opcode |   rd   |  rs1   |  rs2   |  imm  |
```

- `opcode`: 8 bits
- `rd`: 4-bit destination register number
- `rs1`: 4-bit first source register number
- `rs2`: 4-bit second source register number
- `imm`: 44-bit signed immediate

All unused register fields in an instruction must be 0 unless otherwise noted.

### 5.1 Immediate values

The 44-bit `imm` field is **sign-extended** to 64 bits when interpreted as a
signed value. Individual instructions may use the sign-extended value as a
byte offset, an instruction-word offset, or an immediate operand.

Zero-extension is used only for `LOAD8` and `LOAD16` results.

## 6. Instruction set

### 6.1 Opcode map

| Opcode | Mnemonic | Description |
|--------|----------|-------------|
| `0x00` | `HALT` | Stop execution and exit the emulator. |
| `0x01` | `LI` | Load immediate: `rd = sext(imm)` |
| `0x02` | `ADD` | `rd = rs1 + rs2` |
| `0x03` | `ADDI` | `rd = rs1 + sext(imm)` |
| `0x04` | `SUB` | `rd = rs1 - rs2` |
| `0x05` | `AND` | `rd = rs1 & rs2` |
| `0x06` | `OR` | `rd = rs1 | rs2` |
| `0x07` | `LOAD8` | `rd = zero_ext(mem8[rs1 + sext(imm)])` |
| `0x08` | `LOAD64` | `rd = mem64[rs1 + sext(imm)]` |
| `0x09` | `STORE8` | `mem8[rs1 + sext(imm)] = low 8 bits of rs2` |
| `0x0A` | `STORE64` | `mem64[rs1 + sext(imm)] = rs2` |
| `0x0B` | `BEQ` | `if rs1 == rs2 then pc = pc + (sext(imm) * 8)` |
| `0x0C` | `BNE` | `if rs1 != rs2 then pc = pc + (sext(imm) * 8)` |
| `0x0D` | `JMP` | `pc = pc + (sext(imm) * 8)` |
| `0x0E` | `TRAP` | System call: `epc = pc; pc = mem64[0x2000]; x10 = imm` |
| `0x0F` | `JMPR` | `pc = rs1` |
| `0x10` | `ERET` | `pc = epc` |
| `0x11` | `RDCSR` | `rd = CSR[imm & 0xFF]` |
| `0x12` | `WRCSR` | `CSR[imm & 0xFF] = rs1` |
| `0x13` | `SFENCE` | Flush the TLB |

Opcodes `0x14` .. `0xFF` are reserved and cause an illegal-instruction fault
in the MVP.

### 6.2 Detailed semantics

#### `HALT` (0x00)

```
HALT
```

Stops the processor. The emulator exits cleanly.

All register and immediate fields are ignored.

#### `LI` (0x01)

```
LI rd, imm
```

`rd = sign-extend(imm)`

Load a 44-bit signed immediate into a register.

#### `ADD` (0x02)

```
ADD rd, rs1, rs2
```

`rd = rs1 + rs2` (64-bit two's-complement addition)

#### `ADDI` (0x03)

```
ADDI rd, rs1, imm
```

`rd = rs1 + sign-extend(imm)`

#### `SUB` (0x04)

```
SUB rd, rs1, rs2
```

`rd = rs1 - rs2` (64-bit two's-complement subtraction)

#### `AND` (0x05)

```
AND rd, rs1, rs2
```

`rd = rs1 & rs2` (bitwise AND)

#### `OR` (0x06)

```
OR rd, rs1, rs2
```

`rd = rs1 | rs2` (bitwise OR)

#### `LOAD8` (0x07)

```
LOAD8 rd, [rs1 + imm]
```

`rd = zero_extend(mem8[rs1 + sign-extend(imm)])`

Read one byte from memory and zero-extend it to 64 bits.

#### `LOAD64` (0x08)

```
LOAD64 rd, [rs1 + imm]
```

`rd = mem64[rs1 + sign-extend(imm)]`

Read one doubleword (8 bytes, little-endian) from memory.

#### `STORE8` (0x09)

```
STORE8 [rs1 + imm], rs2
```

`mem8[rs1 + sign-extend(imm)] = rs2[7:0]`

Write the low byte of `rs2` to memory.

#### `STORE64` (0x0A)

```
STORE64 [rs1 + imm], rs2
```

`mem64[rs1 + sign-extend(imm)] = rs2`

Write the full 64-bit value of `rs2` to memory, little-endian.

#### `BEQ` (0x0B)

```
BEQ rs1, rs2, imm
```

`if rs1 == rs2 then pc = pc + (sign-extend(imm) * 8)`

The `rd` field is ignored.

#### `BNE` (0x0C)

```
BNE rs1, rs2, imm
```

`if rs1 != rs2 then pc = pc + (sign-extend(imm) * 8)`

The `rd` field is ignored.

#### `JMP` (0x0D)

```
JMP imm
```

`pc = pc + (sign-extend(imm) * 8)`

The `rd`, `rs1`, and `rs2` fields are ignored.

#### `TRAP` (0x0E)

```
TRAP imm
```

System call / trap.

- `epc = pc` (the address of the instruction following the `TRAP`).
- `x10 = sign-extend(imm)` (the syscall number, passed to the handler).
- `pc = mem64[0x2000]` (the trap vector at the fixed address `0x2000`).

The OS must store the address of its trap/syscall handler at `0x2000` before
any user code runs.

#### `ERET` (0x10)

```
ERET
```

Return from a trap or syscall.

`pc = epc`

All register and immediate fields are ignored.

#### `JMPR` (0x0F)

```
JMPR rs1
```

`pc = rs1`

Jump to the address contained in `rs1`. The `rd`, `rs2`, and `imm` fields are
ignored. This is the only way to implement a return from a subroutine or a
function pointer in the MVP.

#### `RDCSR` (0x11)

```
RDCSR rd, csr_imm
```

`rd = CSR[csr_imm & 0xFF]`

Read a control/status register. The CSR number is encoded in the 44-bit
immediate field (only the low 8 bits are used). See section 6.4 for the CSR
map.

#### `WRCSR` (0x12)

```
WRCSR rs1, csr_imm
```

`CSR[csr_imm & 0xFF] = rs1`

Write a control/status register. The `rd` and `rs2` fields are ignored.

#### `SFENCE` (0x13)

```
SFENCE
```

Flush the translation lookaside buffer (TLB). All register and immediate
fields are ignored. This is a no-op when paging is disabled.

### 6.3 Notes on register x0

Because `x0` is hard-wired to zero, `ADDI x0, x0, 0` is a no-op and
`BEQ x1, x0, offset` is a common way to branch when `x1` is zero.

### 6.4 Control/status registers (CSRs)

The CSR file has 256 64-bit entries, accessed via `RDCSR` and `WRCSR`.

| CSR number | Name   | Purpose                                          |
|------------|--------|--------------------------------------------------|
| 0          | `PTBR` | Page table base register. Physical address of the PML4 root table. Must be page-aligned. |
| 1          | `PMODE`| Paging mode. 0 = disabled (flat physical), 1 = enabled (virtual addresses translated). |

CSRs 2..255 are reserved for future use. See `docs/specs/mik-64-paging.md` for
the full paging specification.

## 7. Boot protocol

When the emulator starts it performs the following steps:

1. Allocate 128 MiB of RAM.
2. Zero all of it.
3. Load the flat kernel/user binary at physical address `0x400000`.
4. Set all general-purpose registers to 0.
5. Set the initial `pc` to `0x400000`.
6. Set `x15` (the stack pointer, by convention) to `0x8000000`.
7. Begin fetching and executing instructions at `pc`.

The flat binary is a raw sequence of bytes. The emulator copies the file
directly into memory starting at the load address. Any bytes beyond the end of
the file are zero.

## 8. Memory-mapped I/O

### 8.1 Serial data port

Address: `0x1000`

- **Write:** write the low 8 bits of the store value to the emulator's
  standard output. This is how the kernel prints characters.
- **Read:** returns 0.

### 8.2 Serial status port

Address: `0x1001`

- **Write:** ignored.
- **Read:** returns `0xFF` (always ready to transmit).

All other MMIO/reserved addresses are ignored on write and return 0 on read.

## 9. Exceptions and traps

The `TRAP` instruction is the MVP's system-call mechanism. It stores the
return address in the internal `epc` register, writes the syscall number to
`x10`, and jumps to the address stored at the trap vector `0x2000`. `ERET`
returns to `epc`.

Privilege levels and nested traps are not implemented in the MVP. Illegal
opcodes and out-of-bounds memory accesses halt the emulator with an error.

## 10. Example: "Hello, Mik!"

The following program prints `Hello, Mik!` followed by a newline and then halts.
It is hand-assembled and loaded at `0x400000`.

### Assembly

```
    li   x1, string          ; x1 = address of the string
loop:
    load8 x2, [x1]           ; x2 = next character
    beq  x2, x0, done        ; if x2 == 0, exit loop
    store8 [x0 + 0x1000], x2 ; print character
    addi x1, x1, 1           ; advance pointer
    jmp  loop                ; repeat
done:
    halt
string:
    .asciz "Hello, Mik!\n"
```

### Instruction encoding details

For this example the binary is loaded at `0x400000`, the code is 7 instructions
long (56 bytes), and the string starts at `0x400038`.

| Instruction | Address | `opcode` | `rd` | `rs1` | `rs2` | `imm` (decimal) |
|-------------|---------|----------|------|-------|-------|-----------------|
| `li x1, 0x400038` | `0x400000` | `0x01` | 1 | 0 | 0 | `0x400038` (4194360) |
| `load8 x2, [x1]` | `0x400008` | `0x07` | 2 | 1 | 0 | 0 |
| `beq x2, x0, +4` | `0x400010` | `0x0B` | 0 | 2 | 0 | 4 |
| `store8 [x0 + 0x1000], x2` | `0x400018` | `0x09` | 0 | 0 | 2 | `0x1000` (4096) |
| `addi x1, x1, 1` | `0x400020` | `0x03` | 1 | 1 | 0 | 1 |
| `jmp -4` | `0x400028` | `0x0D` | 0 | 0 | 0 | -4 |
| `halt` | `0x400030` | `0x00` | 0 | 0 | 0 | 0 |

The string bytes (`"Hello, Mik!\n"` plus a null byte) follow immediately at
`0x400038`.

A tiny helper to produce the binary from this table is included in the emulator
build so the example can be exercised automatically.
