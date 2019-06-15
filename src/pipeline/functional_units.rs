use super::load_buffer::LoadBufferEntry;
use super::reorder_buffer::ReorderBufferEntry;
use super::reservation_staion::RSEntry;
use instruction::{Function, Instruction, Opcode};

#[derive(Debug)]
pub struct FunctionalUnitEntry {
    remain_clock: usize,
    func: Function,
    A: u32,
    B: u32,
}

#[derive(Debug, Default)]
pub struct FunctionalUnits {
    buf: std::collections::HashMap<usize, FunctionalUnitEntry>,
}

impl FunctionalUnits {
    fn remain_clocks(inst: &Instruction) -> usize {
        match inst.opcode {
            Opcode::Store | Opcode::Load | Opcode::Amo => 10,
            _ => match inst.function {
                Function::Mul | Function::Mulh | Function::Mulhsu | Function::Mulhu => 4,
                Function::Div | Function::Divu | Function::Rem | Function::Remu => 8,
                _ => 1,
            },
        }
    }

    pub fn execute_general(&mut self, entry: &RSEntry) -> Option<u32> {
        let unit_entry = self.buf.entry(entry.rob_index).or_insert_with(|| {
            let (A, B) = entry.operand_values();
            FunctionalUnitEntry {
                remain_clock: Self::remain_clocks(&entry.inst),
                func: entry.inst.function,
                A: A.unwrap(),
                B: B.unwrap(),
            }
        });

        unit_entry.remain_clock -= 1;
        if unit_entry.remain_clock <= 0 {
            Some(crate::alu::alu(
                &unit_entry.func,
                unit_entry.A as i32,
                unit_entry.B as i32,
            ) as u32)
        } else {
            None
        }
    }

    pub fn execute_load(&mut self, entry: &LoadBufferEntry) -> Option<u32> {
        let unit_entry = self.buf.entry(entry.rob_index).or_insert_with(|| {
            let (A, B) = (entry.addr, entry.value);
            FunctionalUnitEntry {
                remain_clock: 10,
                func: entry.func,
                A: A,
                B: B,
            }
        });
        // rob에 같은 주소의 store가 있으면 skip
        unimplemented!()
    }

    pub fn execute_store(&mut self, entry: &ReorderBufferEntry) -> Option<()> {
        // TODO: Store가 아니면 무조건 성공
        // 같은 주소의 load가 실행중이면 None 반환
        unimplemented!()
    }
}
