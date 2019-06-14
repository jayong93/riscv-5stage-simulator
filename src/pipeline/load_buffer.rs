use std::collections::VecDeque;
use super::operand::Operand;
use super::reorder_buffer::{ReorderBuffer, MetaData};
use register::RegisterFile;

#[derive(Debug)]
pub struct LoadBufferEntry {
    rob_index: usize,
    base: Operand,
    addr: u32       // initially has imm value.
}

#[derive(Debug, Default)]
pub struct LoadBuffer {
    buf: VecDeque<LoadBufferEntry>,
}

impl LoadBuffer {
    pub fn clear(&mut self) {
        self.buf.clear();
    }

    pub fn issue(&mut self, rob_index: usize, rob: &ReorderBuffer, reg: &RegisterFile) {
        let inst = rob.get(rob_index).unwrap().inst.clone();
        let base_reg = inst.fields.rs1.unwrap() as usize;
        let base = {
            let val = reg.gpr[base_reg].read();
            rob.iter().enumerate().rev().find_map(|(rel_pos, entry)| {
                if let MetaData::Normal(reg) = entry.meta {
                    if reg as usize == base_reg {
                        return rob.to_index(rel_pos);
                    }
                }
                None
            }).map_or(Operand::Value(val), |idx| Operand::Rob(idx))
        };

        let new_entry = LoadBufferEntry {
            rob_index,
            base,
            addr: inst.fields.imm.unwrap(),
        };
        self.buf.push_back(new_entry);
    }

    pub fn pop_finished(&mut self) -> Vec<LoadBufferEntry> {
        unimplemented!()
    }
}