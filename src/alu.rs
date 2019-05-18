//! Arithmetic logic unit.

use instruction::Function;

/// Perform one ALU operation.
pub fn alu(func: &Function, input1: i32, input2: i32) -> i32 {
    use instruction::Function::*;

    match &func {
        Add | Addi | AuiPc | Jal | Jalr => input1.wrapping_add(input2),
        Sub => input1.wrapping_sub(input2),
        Slt | Slti => {
            if input1 < input2 {
                1
            } else {
                0
            }
        }
        Sltu | Sltiu => {
            if (input1 as u32) < (input2 as u32) {
                1
            } else {
                0
            }
        }
        And | Andi => input1 & input2,
        Or | Ori => input1 | input2,
        Xor | Xori => input1 ^ input2,
        Sll => ((input1 as u32) << (input2 as u32 & 0x1f)) as i32,
        Slli => ((input1 as u32) << (input2 as u32)) as i32,
        Srl => ((input1 as u32) >> (input2 as u32 & 0x1f)) as i32,
        Srli => ((input1 as u32) >> (input2 as u32)) as i32,
        Sra => input1 >> (input2 as u32 & 0x1f),
        Srai => input1 >> (input2 as u32),
        Lui => input2,
        Beq => (input1 == input2) as i32,
        Bne => (input1 != input2) as i32,
        Blt => (input1 < input2) as i32,
        Bltu => ((input1 as u32) < (input2 as u32)) as i32,
        Bge => (input1 >= input2) as i32,
        Bgeu => ((input1 as u32) >= (input2 as u32)) as i32,
        Lb | Lbu | Lh | Lhu | Lw | Sb | Sh | Sw => input1 + input2,
        Mul => (((input1 as i64) * (input1 as i64)) & 0xffffffff) as i32,
        Mulh => ((((input1 as i64) * (input2 as i64)) as u64) >> 32) as i32,
        Mulhu | Mulhsu => (((input1 as u64) * (input2 as u64)) >> 32) as i32,
        Div => input1 / input2,
        Divu => ((input1 as u32).wrapping_div(input2 as u32)) as i32,
        Rem => input1.wrapping_rem(input2),
        Remu => ((input1 as u32).wrapping_rem(input2 as u32)) as i32,
        _ => 0,
    }
}
