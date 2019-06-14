use instruction::Instruction;

#[derive(Debug, Clone)]
pub enum MetaData {
    Branch { pred_taken: bool, is_taken: bool },
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
    pub is_ready: bool,
}

impl ReorderBufferEntry {
    pub fn new(pc: u32, inst: Instruction) -> Self {
        use instruction::Function;
        use instruction::Opcode::*;

        let (meta, is_ready) = match inst.opcode {
            Branch => (
                MetaData::Branch {
                    pred_taken: false,
                    is_taken: false,
                },
                false,
            ),
            Store => (MetaData::Store(0), false),
            System if inst.function == Function::Ecall => (MetaData::Syscall, true),
            _ => (
                MetaData::Normal(inst.fields.rd.unwrap_or(0)),
                if let Function::Jal = inst.function {
                    true
                } else {
                    false
                },
            ),
        };
        let value = match inst.opcode {
            Jal | Jalr => pc + crate::consts::WORD_SIZE as u32,
            Branch => pc + inst.fields.imm.unwrap(),
            _ => 0,
        };
        ReorderBufferEntry {
            pc,
            inst,
            meta,
            value,
            is_ready,
        }
    }
}

pub struct Iter<'a> {
    rob: &'a Vec<ReorderBufferEntry>,
    head: usize,
    tail: usize,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a ReorderBufferEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.head == self.tail {
            return None;
        }
        let old_head = self.head;
        self.head = (self.head + 1) % self.rob.len();
        self.rob.get(old_head)
    }
}

impl<'a> ExactSizeIterator for Iter<'a> {
    fn len(&self) -> usize {
        let tail = if self.tail < self.head {
            self.tail + self.rob.len()
        } else {
            self.tail
        };
        tail - self.head
    }
}

impl<'a> DoubleEndedIterator for Iter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.head == self.tail {
            return None;
        }
        let old_tail = self.tail;
        self.tail = if self.tail == 0 {
            self.rob.len() - 1
        } else {
            self.tail - 1
        };
        self.rob.get(old_tail)
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

    pub fn retire(&mut self) -> Vec<ReorderBufferEntry> {
        let retired_entries: Vec<_> = self.iter().filter(|entry| entry.is_ready).cloned().collect();
        self.head = (self.head + retired_entries.len()) % self.buf.len();
        retired_entries
    }

    pub fn issue(&mut self, pc: u32, inst: Instruction) -> usize {
        let real_len = self.buf.len() - 1;
        if real_len <= self.len() {
            if real_len == 0 {
                self.buf.resize_with(2, Default::default);
            } else {
                self.buf.resize_with(real_len * 2 + 1, Default::default);
            }
        }

        self.buf[self.tail] = ReorderBufferEntry::new(pc, inst);
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

    pub fn iter(&self) -> Iter {
        Iter {
            rob: &self.buf,
            tail: self.tail,
            head: self.head,
        }
    }
}
