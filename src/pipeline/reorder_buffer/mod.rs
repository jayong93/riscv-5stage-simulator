pub mod iter;
use super::operand::Operand;
use instruction::{Instruction, Opcode};
use pipeline::reservation_staion::FinishedCalc;

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
            _ => None,
        };
        let rd = inst.fields.rd.unwrap_or(0);
        let mut new_entry = ReorderBufferEntry {
            pc,
            inst,
            reg_value: None,
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
        unimplemented!()
    }
}
