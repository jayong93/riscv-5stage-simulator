//! Instruction decode stage.

use consts;

/// A single machine instruction.
#[derive(Clone, Debug)]
pub struct Instruction {
    pub value: u32,

    /// Category of the instruction, e.g., load, branch, or op
    pub opcode: Opcode,

    /// Format associated with the opcode, e.g., R-type or I-type
    pub format: Format,

    /// Struct for accessing the subfields' bits
    pub fields: Fields,

    /// Instruction's mnemonic, e.g., JAL, XOR, or SRA
    pub function: Function,
}

impl Instruction {
    /// Constructs a new `Instruction`.
    pub fn new(value: u32) -> Instruction {
        // convert unnecessary instruction to NOP
        if let 0x003027f3 | 0x00351073 = value {
            return Default::default();
        }

        let opcode: Opcode = value.into();
        let format = opcode.into();
        let fields = Fields::new(value, format, opcode);
        let function = Function::new(value, &fields, opcode);
        Instruction {
            value,
            opcode,
            format,
            fields,
            function,
        }
    }

    pub fn is_nop(&self) -> bool {
        self.value == consts::NOP
    }
}

impl Default for Instruction {
    /// Constructs a canonical NOP encoded as ADDI x0, x0, 0.
    fn default() -> Instruction {
        Instruction::new(consts::NOP)
    }
}

/// RISC-V 32I fields (shamt -> imm).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fields {
    pub rs1: Option<u8>,
    pub rs2: Option<u8>,
    pub rs3: Option<u8>,
    pub rd: Option<u8>,
    pub funct2: Option<u8>,
    pub funct3: Option<u8>,
    pub funct7: Option<u8>,
    pub imm: Option<u32>,
}

impl std::fmt::Display for Fields {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "[ ")?;
        if let Some(rs1) = self.rs1 {
            write!(f, "rs1: {}, ", rs1)?;
        }
        if let Some(rs2) = self.rs2 {
            write!(f, "rs2: {}, ", rs2)?;
        }
        if let Some(rs3) = self.rs3 {
            write!(f, "rs3: {}, ", rs3)?;
        }
        if let Some(rd) = self.rd {
            write!(f, "rd: {}, ", rd)?;
        }
        if let Some(imm) = self.imm {
            write!(f, "imm: {:x}, ", imm)?;
        }
        write!(f, "]")
    }
}

impl Fields {
    pub fn new(inst: u32, format: Format, opcode: Opcode) -> Self {
        use consts::*;
        let rs1 = ((inst & RS1_MASK) >> RS1_SHIFT) as u8;
        let rs2 = ((inst & RS2_MASK) >> RS2_SHIFT) as u8;
        let rs3 = ((inst & RS3_MASK) >> RS3_SHIFT) as u8;
        let rd = ((inst & RD_MASK) >> RD_SHIFT) as u8;
        let funct2 = ((inst & FUNCT2_MASK) >> FUNCT2_SHIFT) as u8;
        let funct3 = ((inst & FUNCT3_MASK) >> FUNCT3_SHIFT) as u8;
        let funct7 = ((inst & FUNCT7_MASK) >> FUNCT7_SHIFT) as u8;
        let imm = match format {
            Format::R => 0,
            Format::I if opcode == Opcode::OpImm && (funct3 == 0x1 || funct3 == 0x5) => {
                (inst & RS2_MASK) >> RS2_SHIFT
            }
            Format::I => (inst & 0xfff00000) >> 20,
            Format::S => ((inst & 0xfe000000) >> 20) | ((inst & 0xf80) >> 7),
            Format::B => {
                ((inst & 0x80000000) >> 19)
                    | ((inst & 0x80) << 4)
                    | ((inst & 0x7e000000) >> 20)
                    | ((inst & 0xf00) >> 7)
            }
            Format::U => inst & 0xfffff000,
            Format::J => {
                (((inst & 0x80000000) >> 11)
                    | (inst & 0xff000)
                    | ((inst & 0x100000) >> 9)
                    | ((inst & 0x7fe00000) >> 20))
            }
            _ => 0,
        };
        let shamt = match opcode {
            Opcode::Lui | Opcode::AuiPc => 0,
            Opcode::Jal => 11,
            Opcode::Jalr => 20,
            Opcode::Branch => 19,
            _ => 20,
        };
        let imm = (((imm as i32) << shamt) >> shamt) as u32;

        let (rs1, rs2, rs3, rd, funct2, funct3, funct7, imm) = match format {
            Format::R => (
                Some(rs1),
                Some(rs2),
                None,
                Some(rd),
                None,
                Some(funct3),
                Some(funct7),
                None,
            ),
            Format::R4 => (
                Some(rs1),
                Some(rs2),
                Some(rs3),
                Some(rd),
                Some(funct2),
                Some(funct3),
                None,
                None,
            ),
            Format::I => (
                Some(rs1),
                None,
                None,
                Some(rd),
                None,
                Some(funct3),
                None,
                Some(imm),
            ),
            Format::S | Format::B => (
                Some(rs1),
                Some(rs2),
                None,
                None,
                None,
                Some(funct3),
                None,
                Some(imm),
            ),
            Format::J | Format::U => (None, None, None, Some(rd), None, None, None, Some(imm)),
        };

        Fields {
            rs1,
            rs2,
            rs3,
            rd,
            funct2,
            funct3,
            funct7,
            imm,
        }
    }
}

/// RISC-V 32I opcodes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Opcode {
    Lui,
    AuiPc,
    Jal,
    Jalr,
    Branch,
    Load,
    Store,
    Op,
    OpImm,
    MiscMem,
    System,
    Amo,
    LoadFp,
    StoreFp,
    OpFp,
    Fmadd,
    Fmsub,
    Fnmadd,
    Fnmsub,
}

impl From<u32> for Opcode {
    fn from(val: u32) -> Self {
        let opcode = val & consts::OPCODE_MASK;
        match opcode {
            0b01_101_11 => Opcode::Lui,
            0b00_101_11 => Opcode::AuiPc,
            0b11_011_11 => Opcode::Jal,
            0b11_001_11 => Opcode::Jalr,
            0b11_000_11 => Opcode::Branch,
            0b00_000_11 => Opcode::Load,
            0b01_000_11 => Opcode::Store,
            0b01_100_11 => Opcode::Op,
            0b00_100_11 => Opcode::OpImm,
            0b00_011_11 => Opcode::MiscMem,
            0b11_100_11 => Opcode::System,
            0b01_011_11 => Opcode::Amo,
            0b00_001_11 => Opcode::LoadFp,
            0b01_001_11 => Opcode::StoreFp,
            0b10_000_11 => Opcode::Fmadd,
            0b10_001_11 => Opcode::Fmsub,
            0b10_010_11 => Opcode::Fnmsub,
            0b10_011_11 => Opcode::Fnmadd,
            0b10_100_11 => Opcode::OpFp,
            _ => panic!("Unknown opcode {:#09b}", opcode),
        }
    }
}

/// RISC-V 32I instruction formats.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Format {
    R,
    R4,
    I,
    S,
    B,
    U,
    J,
}

impl From<Opcode> for Format {
    fn from(opcode: Opcode) -> Self {
        match opcode {
            Opcode::Lui => Format::U,
            Opcode::AuiPc => Format::U,
            Opcode::Jal => Format::J,
            Opcode::Jalr => Format::I,
            Opcode::Branch => Format::B,
            Opcode::Load | Opcode::LoadFp => Format::I,
            Opcode::Store | Opcode::StoreFp => Format::S,
            Opcode::Op | Opcode::OpFp => Format::R,
            Opcode::OpImm => Format::I,
            Opcode::MiscMem => Format::I,
            Opcode::System => Format::I,
            Opcode::Fmadd | Opcode::Fmsub | Opcode::Fnmadd | Opcode::Fnmsub | Opcode::Amo => {
                Format::R4
            }
        }
    }
}

/// RISC-V 32I mnemonics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Function {
    /// Load upper immediate
    Lui,
    /// Add upper immediate to PC
    AuiPc,
    // Jumps
    /// Jump and link
    Jal,
    /// Jump and link register
    Jalr,
    // Branches
    /// Branch if equal
    Beq,
    /// Branch if not equal
    Bne,
    /// Branch if less than
    Blt,
    /// Branch if greater or equal
    Bge,
    /// Branch if less than (unsigned)
    Bltu,
    /// Branch if greater or equal (unsigned)
    Bgeu,
    // Loads
    /// Load byte
    Lb,
    /// Load halfword
    Lh,
    /// Load word
    Lw,
    /// Load byte (unsigned)
    Lbu,
    /// Load halfword (unsigned)
    Lhu,
    // Stores
    /// Store byte
    Sb,
    /// Store halfword
    Sh,
    /// Store word
    Sw,
    // Operations on immediates
    /// Add immediate
    Addi,
    /// Set less than immediate
    Slti,
    /// Set less than immediate (unsigned)
    Sltiu,
    /// Exclusive or immediate
    Xori,
    /// Logical Or immediate
    Ori,
    /// Logical And immediate
    Andi,
    /// Shift left logical immediate
    Slli,
    /// Shift right logical immediate
    Srli,
    /// Shift right arithmetic immediate
    Srai,
    // Operations on registers
    /// Add
    Add,
    /// Subtract
    Sub,
    /// Shift left logical
    Sll,
    /// Set less than
    Slt,
    /// Set less than unsigned
    Sltu,
    /// Exclusive or
    Xor,
    /// Shift right logical
    Srl,
    /// Shift right arithmetic
    Sra,
    /// Logical Or
    Or,
    /// Logical And
    And,
    Fence,
    Fencei,
    Ecall,
    Ebreak,
    Mul,
    Mulh,
    Mulhsu,
    Mulhu,
    Div,
    Divu,
    Rem,
    Remu,
    Lrw,
    Scw,
    Amoswapw,
    Amoaddw,
    Amoxorw,
    Amoandw,
    Amoorw,
    Amominw,
    Amomaxw,
    Amominuw,
    Amomaxuw,
    Flw,
    Fsw,
    Fmadds,
    Fmsubs,
    Fnmsubs,
    Fnmadds,
    Fadds,
    Fsubs,
    Fmuls,
    Fdivs,
    Fsqrts,
    Fsgnjs,
    Fsgnjns,
    Fsgnjxs,
    Fmins,
    Fmaxs,
    Fcvtws,
    Fcvtwus,
    Fmvxw,
    Feqs,
    Flts,
    Fles,
    Fclasss,
    Fcvtsw,
    Fcvtswu,
    Fmvwx,
}

impl Function {
    pub fn new(inst: u32, fields: &Fields, opcode: Opcode) -> Function {
        // Check opcode-only functions
        match opcode {
            Opcode::Lui => Function::Lui,
            Opcode::AuiPc => Function::AuiPc,
            Opcode::Jal => Function::Jal,
            Opcode::Jalr => Function::Jalr,
            Opcode::LoadFp => Function::Flw,
            Opcode::StoreFp => Function::Fsw,
            Opcode::Fmadd => Function::Fmadds,
            Opcode::Fmsub => Function::Fmsubs,
            Opcode::Fnmadd => Function::Fnmadds,
            Opcode::Fnmsub => Function::Fnmsubs,
            _ => {
                // Check rest of functions
                match (opcode, fields.funct3, fields.funct7) {
                    (Opcode::Branch, Some(0b000), _) => Function::Beq,
                    (Opcode::Branch, Some(0b001), _) => Function::Bne,
                    (Opcode::Branch, Some(0b100), _) => Function::Blt,
                    (Opcode::Branch, Some(0b101), _) => Function::Bge,
                    (Opcode::Branch, Some(0b110), _) => Function::Bltu,
                    (Opcode::Branch, Some(0b111), _) => Function::Bgeu,
                    (Opcode::Load, Some(0b000), _) => Function::Lb,
                    (Opcode::Load, Some(0b001), _) => Function::Lh,
                    (Opcode::Load, Some(0b010), _) => Function::Lw,
                    (Opcode::Load, Some(0b100), _) => Function::Lbu,
                    (Opcode::Load, Some(0b101), _) => Function::Lhu,
                    (Opcode::Store, Some(0b000), _) => Function::Sb,
                    (Opcode::Store, Some(0b001), _) => Function::Sh,
                    (Opcode::Store, Some(0b010), _) => Function::Sw,
                    (Opcode::OpImm, Some(0b000), _) => Function::Addi,
                    (Opcode::OpImm, Some(0b010), _) => Function::Slti,
                    (Opcode::OpImm, Some(0b011), _) => Function::Sltiu,
                    (Opcode::OpImm, Some(0b100), _) => Function::Xori,
                    (Opcode::OpImm, Some(0b110), _) => Function::Ori,
                    (Opcode::OpImm, Some(0b111), _) => Function::Andi,
                    (Opcode::OpImm, Some(0b001), _) => Function::Slli,
                    (Opcode::OpImm, Some(0b101), _)
                        if (inst & consts::FUNCT7_MASK) >> consts::FUNCT7_SHIFT == 0 =>
                    {
                        Function::Srli
                    }
                    (Opcode::OpImm, Some(0b101), _)
                        if (inst & consts::FUNCT7_MASK) >> consts::FUNCT7_SHIFT == 0b01_00000 =>
                    {
                        Function::Srai
                    }
                    (Opcode::Op, Some(0b000), Some(0b0)) => Function::Add,
                    (Opcode::Op, Some(0b000), Some(0b01_00000)) => Function::Sub,
                    (Opcode::Op, Some(0b001), Some(0b0)) => Function::Sll,
                    (Opcode::Op, Some(0b010), Some(0b0)) => Function::Slt,
                    (Opcode::Op, Some(0b011), Some(0b0)) => Function::Sltu,
                    (Opcode::Op, Some(0b100), Some(0b0)) => Function::Xor,
                    (Opcode::Op, Some(0b101), Some(0b0)) => Function::Srl,
                    (Opcode::Op, Some(0b101), Some(0b01_00000)) => Function::Sra,
                    (Opcode::Op, Some(0b110), Some(0b0)) => Function::Or,
                    (Opcode::Op, Some(0b111), Some(0b0)) => Function::And,
                    (Opcode::MiscMem, Some(0b000), _) => Function::Fence,
                    (Opcode::MiscMem, Some(0b001), _) => Function::Fencei,
                    (Opcode::System, Some(0b0), _) if fields.imm == Some(1) => Function::Ebreak,
                    (Opcode::System, Some(0b0), _) => Function::Ecall,
                    (Opcode::Op, Some(0b000), Some(0b1)) => Function::Mul,
                    (Opcode::Op, Some(0b001), Some(0b1)) => Function::Mulh,
                    (Opcode::Op, Some(0b010), Some(0b1)) => Function::Mulhsu,
                    (Opcode::Op, Some(0b011), Some(0b1)) => Function::Mulhu,
                    (Opcode::Op, Some(0b100), Some(0b1)) => Function::Div,
                    (Opcode::Op, Some(0b101), Some(0b1)) => Function::Divu,
                    (Opcode::Op, Some(0b110), Some(0b1)) => Function::Rem,
                    (Opcode::Op, Some(0b111), Some(0b1)) => Function::Remu,
                    (Opcode::Amo, Some(0b010), _) if fields.rs3 == Some(0b00010) => Function::Lrw,
                    (Opcode::Amo, Some(0b010), _) if fields.rs3 == Some(0b00011) => Function::Scw,
                    (Opcode::Amo, Some(0b010), _) if fields.rs3 == Some(0b00001) => {
                        Function::Amoswapw
                    }
                    (Opcode::Amo, Some(0b010), _) if fields.rs3 == Some(0b00000) => {
                        Function::Amoaddw
                    }
                    (Opcode::Amo, Some(0b010), _) if fields.rs3 == Some(0b00100) => {
                        Function::Amoxorw
                    }
                    (Opcode::Amo, Some(0b010), _) if fields.rs3 == Some(0b01100) => {
                        Function::Amoandw
                    }
                    (Opcode::Amo, Some(0b010), _) if fields.rs3 == Some(0b01000) => {
                        Function::Amoorw
                    }
                    (Opcode::Amo, Some(0b010), _) if fields.rs3 == Some(0b10000) => {
                        Function::Amominw
                    }
                    (Opcode::Amo, Some(0b010), _) if fields.rs3 == Some(0b10100) => {
                        Function::Amomaxw
                    }
                    (Opcode::Amo, Some(0b010), _) if fields.rs3 == Some(0b11000) => {
                        Function::Amominuw
                    }
                    (Opcode::Amo, Some(0b010), _) if fields.rs3 == Some(0b11100) => {
                        Function::Amomaxuw
                    }
                    (Opcode::OpFp, _, Some(0b0)) => Function::Fadds,
                    (Opcode::OpFp, _, Some(0b100)) => Function::Fsubs,
                    (Opcode::OpFp, _, Some(0b1000)) => Function::Fmuls,
                    (Opcode::OpFp, _, Some(0b1100)) => Function::Fdivs,
                    (Opcode::OpFp, _, Some(0b010_1100)) if fields.rs2 == Some(0b0) => {
                        Function::Fsqrts
                    }
                    (Opcode::OpFp, Some(0b000), Some(0b001_0000)) => Function::Fsgnjs,
                    (Opcode::OpFp, Some(0b001), Some(0b001_0000)) => Function::Fsgnjns,
                    (Opcode::OpFp, Some(0b010), Some(0b001_0000)) => Function::Fsgnjxs,
                    (Opcode::OpFp, Some(0b000), Some(0b001_0100)) => Function::Fmins,
                    (Opcode::OpFp, Some(0b001), Some(0b001_0100)) => Function::Fmaxs,
                    (Opcode::OpFp, _, Some(0b110_0000)) if fields.rs2 == Some(0b1) => {
                        Function::Fcvtwus
                    }
                    (Opcode::OpFp, _, Some(0b110_0000)) => Function::Fcvtws,
                    (Opcode::OpFp, Some(0b000), Some(0b111_0000)) if fields.rs2 == Some(0b0) => {
                        Function::Fmvxw
                    }
                    (Opcode::OpFp, Some(0b010), Some(0b101_0000)) => Function::Feqs,
                    (Opcode::OpFp, Some(0b001), Some(0b101_0000)) => Function::Flts,
                    (Opcode::OpFp, Some(0b000), Some(0b101_0000)) => Function::Fles,
                    (Opcode::OpFp, Some(0b001), Some(0b111_0000)) if fields.rs2 == Some(0b0) => {
                        Function::Fclasss
                    }
                    (Opcode::OpFp, _, Some(0b110_1000)) if fields.rs2 == Some(0b1) => {
                        Function::Fcvtswu
                    }
                    (Opcode::OpFp, _, Some(0b110_1000)) => Function::Fcvtsw,
                    (Opcode::OpFp, Some(0b000), Some(0b111_1000)) if fields.rs2 == Some(0b0) => {
                        Function::Fmvwx
                    }
                    _ => panic!(
                        "Failed to decode instruction {:#0x}, fields: {:x?}",
                        inst, fields
                    ),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Instruction::default() should be a NOP
    #[test]
    fn nop() {
        let insn = Instruction::default();
        assert_eq!(insn.function, Function::Addi);
        assert_eq!(insn.fields.rd, Some(0));
        assert_eq!(insn.fields.rs1, Some(0));
        assert_eq!(insn.fields.imm, Some(0));
    }

}
