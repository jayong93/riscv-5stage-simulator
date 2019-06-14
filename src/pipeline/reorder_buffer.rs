use std::collections::VecDeque;
use instruction::Instruction;

#[derive(Debug, Clone)]
pub enum MetaData {
    Branch(bool),
    Store(u32),
    Syscall,
    Normal(u8),
}

impl Default for MetaData {
    fn default() -> Self {
        MetaData::Normal(0)
    }
}

#[derive(Debug, Default, Clone)]
pub struct ReorderBufferEntry {
    pub pc: u32,
    pub inst: Instruction,
    pub meta: MetaData,
    pub value: u32,
    is_ready: bool,
}

impl ReorderBufferEntry {
    pub fn new(pc: u32, inst: Instruction) -> Self {
        use instruction::Opcode::*;
        use instruction::Function;
        
        let (meta, is_ready) = match inst.opcode {
            Branch => (MetaData::Branch(false), false),
            Store => (MetaData::Store(0), false),
            System if inst.function == Function::Ecall => (MetaData::Syscall, true),
            _ => (MetaData::Normal(inst.fields.rd.unwrap_or(0)), false),
        };
        ReorderBufferEntry {
            pc,
            inst,
            meta, 
            value: 0,
            is_ready,
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
        ReorderBuffer{
            buf: vec![ReorderBufferEntry::default()],
            head: 0,
            tail: 0,
        }
    }
}

impl ReorderBuffer {
    pub fn retire(&mut self) -> Vec<ReorderBufferEntry> {
        unimplemented!()
    }

    pub fn issue(&mut self, pc: u32, inst: Instruction) -> usize {
        let real_len = self.buf.len()-1;
        if real_len <= self.len() {
            if real_len == 0 {
                self.buf.resize_with(2, Default::default);
            } else {
                self.buf.resize_with(real_len * 2 +1, Default::default);
            }
        }

        self.buf[self.tail] = ReorderBufferEntry::new(pc, inst);
        let rob_index = self.tail;
        self.tail = (self.tail+1) % self.buf.len();
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

    pub fn relative_pos(&self, index: usize) -> Option<usize> {
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

    pub fn iter(&self) -> impl std::iter::Iterator<Item=&ReorderBufferEntry> {
        self.buf.iter().cycle().skip(self.head).take(self.len())
    }
}
