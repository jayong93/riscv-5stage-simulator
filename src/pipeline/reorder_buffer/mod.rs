pub mod iter;
use super::functional_units::FunctionalUnits;
use super::operand::Operand;
use instruction::{Instruction, Opcode};

#[derive(Debug, Clone)]


#[derive(Debug, Default, Clone)]
pub struct ReorderBufferEntry {
    pub pc: u32,
    pub inst: Instruction,
    pub mem_value: Operand,
    pub reg_value: u32,
    pub is_completed: bool,
    pub rd: u8,
    pub addr: Operand,
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

    fn get_op_for_store(
        &self,
        inst: &Instruction,
        reg: &crate::register::RegisterFile,
    ) -> (Operand, Operand) {
        let (reg1, reg2) = (
            inst.fields.rs1.unwrap() as usize,
            inst.fields.rs2.unwrap() as usize,
        );
        let (mut op1, mut op2) = (None, None);

        for (idx, entry) in self
            .iter()
            .enumerate()
            .rev()
            .map(|(rel_pos, entry)| (self.to_index(rel_pos).unwrap(), entry))
        {
            if let MetaData::Normal(target_reg) = entry.meta {
                if target_reg as usize == reg1 {
                    op1 = Some(idx);
                } else if target_reg as usize == reg2 {
                    op2 = Some(idx);
                }
            }

            if let (Some(_), Some(_)) = (op1, op2) {
                break;
            }
        }

        let idx_to_op = |idx| {
            let entry = self.get(idx).unwrap();
            if entry.is_ready {
                Operand::Value(entry.value)
            } else {
                Operand::Rob(idx)
            }
        };
        let op1 = op1
            .map(idx_to_op)
            .unwrap_or_else(|| Operand::Value(reg.gpr[reg1].read()));
        let op2 = op2
            .map(idx_to_op)
            .unwrap_or_else(|| Operand::Value(reg.gpr[reg2].read()));
        (op1, op2)
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
            Store => (reg.) 
        };
        let mut new_entry = ReorderBufferEntry{
            pc,
            inst,
            value: 0,
            is_completed: false,

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

    pub fn propagate(&mut self, rob_idx: usize, value: u32) {
        for idx in self
            .iter()
            .enumerate()
            .map(|(i, _)| self.to_index(i).unwrap())
            .collect::<Vec<_>>()
        {
            let entry = self.get_mut(idx).unwrap();
            match entry.meta {
                MetaData::Store(op1, has_value) => {
                    let mut new_op1 = op1;
                    if let Operand::Rob(base_idx) = op1 {
                        if base_idx == rob_idx {
                            new_op1 = Operand::Value(entry.inst.fields.imm.unwrap() + value);
                        }
                    }

                    let new_has_val = if !has_value && entry.value as usize == rob_idx {
                        entry.value = value;
                        true
                    } else {
                        false
                    };
                    entry.meta = MetaData::Store(new_op1, new_has_val);
                }
                _ => {}
            }
        }
    }
}
