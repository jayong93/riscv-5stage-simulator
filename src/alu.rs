//! Arithmetic logic unit.

use pipeline;

/// Perform one ALU operation.
pub fn alu(pipeline_reg: &pipeline::IdExRegister) -> i32 {
    use instruction::Function::*;
    let (src1, src2) = (pipeline_reg.rs1, pipeline_reg.rs2);

    match pipeline_reg.insn.function {
        Add | Addi => src1 + src2,
        Subi => src1 - src2,
        Slt | Slti => {
            if src1 < src2 {
                1
            } else {
                0
            }
        }
        Sltu | Sltiu => {
            if (src1 as u32) < (src2 as u32) {
                1
            } else {
                0
            }
        }
        And | Andi => src1 & src2,
        Or | Ori => src1 | src2,
        Xor | Xori => src1 ^ src2,
        Sll | Slli => ((src1 as u32) << (src2 as u32)) as i32,
        Srl | Srli => ((src1 as u32) >> (src2 as u32)) as i32,
        Sra | Srai => src1 >> src2,
        Lui => src2,
        AuiPc => pipeline_reg.pc as i32 + src2,
        Jalr | Jal => pipeline_reg.pc as i32 + 4,
        Beq => (src1 == src2) as i32,
        Bne => (src1 != src2) as i32,
        Blt => (src1 < src2) as i32,
        Bltu => ((src1 as u32) < (src2 as u32)) as i32,
        Bge => (src1 >= src2) as i32,
        Bgeu => ((src1 as u32) >= (src2 as u32)) as i32,
        Lb | Lbu | Lh | Lhu | Lw | Sb | Sh | Sw => src1 + src2,
        Mul => (((src1 as i64) * (src1 as i64)) & 0xffffffff) as i32,
        Mulh => ((((src1 as i64) * (src2 as i64)) as u64) >> 32) as i32,
        Mulhu | Mulhsu => (((src1 as u64) * (src2 as u64)) >> 32) as i32,
        Div => src1 / src2,
        Divu => ((src1 as u32) / (src2 as u32)) as i32,
        Rem => src1 % src2,
        Remu => ((src1 as u32) % (src2 as u32)) as i32,
        _ => 0
    }
}