//! Global constants

/// Special simulator-only instruction signal to halt simulator.
pub const HALT: u32 = 0x3f;

/// Sizes in bytes.
pub const WORD_SIZE: usize = 4;
pub const HALFWORD_SIZE: usize = 2;
pub const BYTE_SIZE: usize = 1;

/// A canonical RISC-V NOP, encoded as ADDI x0, x0, 0.
pub const NOP: u32 = 0x13;

// Masks to isolate specific parts of the instruction using logical AND (&)
pub const FUNCT7_MASK: u32 = 0xfe000000;
pub const FUNCT3_MASK: u32 = 0x7000;
pub const FUNCT2_MASK: u32 = 0x06000000;
pub const RS1_MASK: u32 = 0xf8000;
pub const RS2_MASK: u32 = 0x1f00000;
pub const RS3_MASK: u32 = 0xf8000000;
pub const RD_MASK: u32 = 0xf80;
pub const OPCODE_MASK: u32 = 0x7f;
pub const BIT30_MASK: u32 = 0x40000000;

// Indices of instruction parts for shifting
pub const FUNCT7_SHIFT: u8 = 25;
pub const FUNCT3_SHIFT: u8 = 12;
pub const FUNCT2_SHIFT: u8 = 25;
pub const RS1_SHIFT: u8 = 15;
pub const RS2_SHIFT: u8 = 20;
pub const RS3_SHIFT: u8 = 27;
pub const RD_SHIFT: u8 = 7;
pub const BIT30_SHIFT: u8 = 30;

pub const SYSCALL_NUM_REG: usize = 17;
pub const SYSCALL_RET_REG: usize = 10;
pub const SYSCALL_ARG1_REG: usize = 10;
pub const SYSCALL_ARG2_REG: usize = 11;
pub const SYSCALL_ARG3_REG: usize = 12;
pub const SYSCALL_ARG4_REG: usize = 13;
pub const SYSCALL_ARG5_REG: usize = 14;
pub const SYSCALL_ARG6_REG: usize = 15;

pub const MEM_CYCLE: usize = 10;
pub const ADD_CYCLE: usize = 1;
pub const MUL_CYCLE: usize = 4;
pub const DIV_CYCLE: usize = 8;
