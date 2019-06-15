use super::load_buffer::LoadBufferEntry;
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
        let result;
        {
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
            result = if unit_entry.remain_clock <= 0 {
                Some(
                    crate::alu::alu(&unit_entry.func, unit_entry.A as i32, unit_entry.B as i32)
                        as u32,
                )
            } else {
                None
            };
        }

        if let Some(_) = result {
            self.buf.remove(&entry.rob_index);
        }
        result
    }

    pub fn execute_load(
        &mut self,
        entry: &LoadBufferEntry,
        mem: &crate::memory::ProcessMemory,
    ) -> Option<u32> {
        let result;
        {
            let unit_entry = self.buf.entry(entry.rob_index).or_insert_with(|| {
                let (A, B) = (entry.addr, 0);
                FunctionalUnitEntry {
                    remain_clock: 10,
                    func: entry.func,
                    A: A,
                    B: B,
                }
            });

            unit_entry.remain_clock -= 1;
            let addr = unit_entry.A;
            result = if unit_entry.remain_clock <= 0 {
                Some(match unit_entry.func {
                    Function::Lb | Function::Lbu => mem.read::<u8>(addr) as u32,
                    Function::Lh | Function::Lhu => mem.read::<u16>(addr) as u32,
                    Function::Lw => mem.read::<u32>(addr),
                    _ => unreachable!(),
                })
            } else {
                None
            };
        }

        if let Some(_) = result {
            self.buf.remove(&entry.rob_index);
        }
        result
    }

    pub fn execute_store(
        &mut self,
        func: Function,
        rob_index: usize,
        addr: u32,
        value: u32,
        mem: &mut crate::memory::ProcessMemory,
    ) -> Option<()> {
        // 같은 주소의 load가 실행중이면 None 반환
        let result;
        {
            let unit_entry = self.buf.entry(rob_index).or_insert_with(|| {
                let (A, B) = (addr, value);
                FunctionalUnitEntry {
                    remain_clock: 10,
                    func: func,
                    A: A,
                    B: B,
                }
            });

            unit_entry.remain_clock -= 1;
            let addr = unit_entry.A;
            result = if unit_entry.remain_clock <= 0 {
                Some(match unit_entry.func {
                    Function::Sb => mem.write(addr, value as u8),
                    Function::Sh => mem.write(addr, value as u16),
                    Function::Sw => mem.write(addr, value as u32),
                    _ => unreachable!(),
                })
            } else {
                None
            };
        }
        result.map(|_| {
            self.buf.remove(&rob_index);
            ()
        })
    }
}
