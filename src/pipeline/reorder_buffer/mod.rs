pub mod iter;
use super::operand::Operand;
use instruction::{Function, Instruction, Opcode};
use memory::ProcessMemory;
use pipeline::reservation_staion::FinishedCalc;
use pipeline::Pipeline;
use register::RegisterFile;

#[derive(Debug, Default, Clone)]
pub struct ReorderBufferEntry {
    pub pc: u32,
    pub inst: Instruction,
    pub mem_value: Operand,
    pub reg_value: Option<u32>,
    pub rd: u8,
    pub addr: Operand,
    pub branch_pred: bool,
    pub mem_rem_cycle: usize,
}

impl ReorderBufferEntry {
    pub fn is_completed(&self) -> bool {
        let mem_val_done = if let Operand::Value(_) = self.mem_value {
            true
        } else {
            false
        };
        let addr_done = if let Operand::Value(addr) = self.addr {
            if addr == 0 {
                false
            } else {
                true
            }
        } else {
            false
        };
        let reg_val_done = self.reg_value.is_some();

        match self.inst.opcode {
            Opcode::Store => mem_val_done && addr_done && self.mem_rem_cycle == 0,
            Opcode::Amo => mem_val_done && addr_done && reg_val_done && self.mem_rem_cycle == 0,
            Opcode::Jalr => addr_done && reg_val_done,
            _ => reg_val_done,
        }
    }

    // true 반환이면 branch prediction miss
    pub fn retire(
        &self,
        old_index: usize,
        memory: &mut ProcessMemory,
        reg: &mut RegisterFile,
    ) -> bool {
        if let Opcode::Branch = self.inst.opcode {
            if let Some(branch_result) = self.reg_value {
                if branch_result == self.branch_pred as u32 {
                    return true;
                }
            }
            return false;
        }

        if let Function::Ecall = self.inst.function {
            Pipeline::system_call(memory, reg).unwrap();
            return false;
        }

        if let Some(reg_val) = self.reg_value {
            reg.gpr[self.rd as usize].write(reg_val);
            if let Some(related_rob) = reg.related_rob[self.rd as usize] {
                if related_rob == old_index {
                    reg.related_rob[self.rd as usize] = None;
                }
            }
        }

        false
    }
}

#[derive(Debug)]
pub struct ReorderBuffer {
    buf: Vec<ReorderBufferEntry>,
    head: usize,
    tail: usize,
}

impl Default for ReorderBuffer {
    fn default() -> Self {
        ReorderBuffer {
            buf: vec![ReorderBufferEntry::default()],
            head: 0,
            tail: 0,
        }
    }
}

impl ReorderBuffer {
    pub fn clear(&mut self) {
        self.head = 0;
        self.tail = 0;
    }

    pub fn issue(
        &mut self,
        pc: u32,
        inst: Instruction,
        reg: &crate::register::RegisterFile,
    ) -> usize {
        let real_len = self.buf.len() - 1;
        if real_len <= self.len() {
            if real_len == 0 {
                self.buf.resize_with(2, Default::default);
            } else {
                self.buf.resize_with(real_len * 2 + 1, Default::default);
            }
        }

        let (mem_value, addr) = match inst.opcode {
            Opcode::Store => (
                reg.get_reg_value(inst.fields.rs2.unwrap()),
                Operand::default(),
            ),
            Opcode::Amo => (
                reg.get_reg_value(inst.fields.rs2.unwrap()),
                reg.get_reg_value(inst.fields.rs1.unwrap()),
            ),
            _ => (Operand::default(), Operand::default()),
        };
        let reg_value = match inst.opcode {
            Opcode::Jal | Opcode::Jalr => Some(pc + crate::consts::WORD_SIZE as u32),
            Opcode::Lui => Some(inst.fields.imm.unwrap()),
            Opcode::AuiPc => Some(pc + inst.fields.imm.unwrap()),
            Opcode::Amo if inst.function == Function::Scw => Some(0),
            _ => None,
        };
        let rd = inst.fields.rd.unwrap_or(0);
        let new_entry = ReorderBufferEntry {
            pc,
            inst,
            reg_value,
            mem_value,
            rd,
            addr,
            branch_pred: false,
            mem_rem_cycle: crate::consts::MEM_CYCLE,
        };

        self.buf[self.tail] = new_entry;
        let rob_index = self.tail;
        self.tail = (self.tail + 1) % self.buf.len();
        rob_index
    }

    pub fn len(&self) -> usize {
        let tail = if self.tail < self.head {
            self.tail + self.buf.len()
        } else {
            self.tail
        };
        tail - self.head
    }

    pub fn to_relative_pos(&self, index: usize) -> Option<usize> {
        self.get(index).and_then(|_| {
            if self.head <= index {
                Some(index - self.head)
            } else if index < self.tail {
                Some(self.len() - (self.tail - index))
            } else {
                None
            }
        })
    }

    pub fn to_index(&self, relative_pos: usize) -> Option<usize> {
        if self.head + relative_pos >= self.buf.len() * 2 {
            None
        } else {
            Some((self.head + relative_pos) % self.buf.len())
        }
    }

    pub fn get(&self, index: usize) -> Option<&ReorderBufferEntry> {
        if index < self.tail || self.head <= index {
            self.buf.get(index)
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut ReorderBufferEntry> {
        if index < self.tail || self.head <= index {
            self.buf.get_mut(index)
        } else {
            None
        }
    }

    pub fn iter(&self) -> iter::Iter {
        iter::Iter {
            rob: &self.buf,
            tail: self.tail,
            head: self.head,
        }
    }

    pub fn propagate(&mut self, job: &FinishedCalc) {
        for rel_pos in 0..self.len() {
            let idx = self.to_index(rel_pos).unwrap();
            let entry = self.get_mut(idx).unwrap();

            if idx == job.rob_idx {
                entry.reg_value = Some(job.reg_value);
                continue;
            }

            let ops = [entry.addr, entry.mem_value];
            let mut it = ops.into_iter().map(|op| match op {
                Operand::Rob(target_rob) if *target_rob == job.rob_idx => {
                    Operand::Value(job.reg_value)
                }
                _ => *op,
            });
            entry.addr = it.next().unwrap();
            entry.mem_value = it.next().unwrap();
        }
    }

    pub fn completed_entries(&mut self) -> Vec<(usize, ReorderBufferEntry)> {
        let completed: Vec<_> = self
            .iter()
            .enumerate()
            .take_while(|(_, entry)| entry.is_completed())
            .map(|(rel_pos, entry)| (self.to_index(rel_pos).unwrap(), entry.clone()))
            .collect();

        self.head = (self.head + completed.len()) % self.buf.len();

        completed
    }
}
