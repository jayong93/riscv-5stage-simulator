use std::collections::VecDeque;
use instruction::Instruction;

#[derive(Debug)]
pub enum MetaData {
    Branch(bool),
    Store(u32),
    Syscall(u32),
    Normal(u8),
}

impl Default for MetaData {
    fn default() -> Self {
        MetaData::Normal(0)
    }
}

#[derive(Debug, Default)]
pub struct ReorderBufferEntry {
    pub pc: u32,
    pub inst: Instruction,
    pub meta: MetaData,
    pub value: u32,
    is_ready: bool,
}

#[derive(Debug, Default)]
pub struct ReorderBuffer {
    buf: VecDeque<ReorderBufferEntry>,
}

impl ReorderBuffer {
    pub fn retire(&mut self) -> Vec<ReorderBufferEntry> {
        unimplemented!()
    }
}