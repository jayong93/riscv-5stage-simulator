use super::functional_units::FunctionalUnits;
use super::operand::Operand;
use super::reorder_buffer::{MetaData, ReorderBuffer};
use register::RegisterFile;
use std::collections::VecDeque;
use instruction::Function;
use memory::ProcessMemory;

#[derive(Debug, Clone)]
pub enum LoadBufferStatus {
    Wait,
    Execute,
    Finished,
}

#[derive(Debug, Clone)]
pub struct LoadBufferEntry {
    pub rob_index: usize,
    pub status: LoadBufferStatus,
    pub func: Function,
    pub base: Operand,
    pub addr: u32, // initially has imm value.
    pub value: u32,
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
            rob.iter()
                .enumerate()
                .rev()
                .find_map(|(rel_pos, entry)| {
                    if let MetaData::Normal(reg) = entry.meta {
                        if reg as usize == base_reg {
                            return rob.to_index(rel_pos);
                        }
                    }
                    None
                })
                .map_or(Operand::Value(val), |idx| Operand::Rob(idx))
        };

        let new_entry = LoadBufferEntry {
            rob_index,
            status: LoadBufferStatus::Wait,
            base,
            func: inst.function,
            addr: inst.fields.imm.unwrap(),
            value: 0,
        };
        self.buf.push_back(new_entry);
    }

    pub fn pop_finished(&mut self) -> Vec<LoadBufferEntry> {
        let finished_num = self
            .buf
            .iter()
            .filter(|entry| {
                if let LoadBufferStatus::Finished = entry.status {
                    true
                } else {
                    false
                }
            })
            .count();

        let mut finished = Vec::new();
        for _ in 0..finished_num {
            finished.push(self.buf.pop_front().unwrap());
        }
        finished
    }

    pub fn propagate(&mut self, rob_index: usize, value: u32) {
        for entry in self.buf.iter_mut() {
            if let Operand::Rob(index) = entry.base {
                if index == rob_index {
                    entry.base = Operand::Value(value);
                    entry.addr += value;
                }
            }
        }
    }

    pub fn execute(&mut self, rob: &ReorderBuffer, func_units: &mut FunctionalUnits, mem: &ProcessMemory) {
        for entry in self.buf.iter_mut().filter(|entry| {
            if let LoadBufferStatus::Finished = entry.status {
                false
            } else {
                true
            }
        }) {
            if let Operand::Value(_) = entry.base {
                let entry_rel_pos = rob.to_relative_pos(entry.rob_index).unwrap();
                if rob.iter().take(entry_rel_pos).any(|rob_entry| {
                    if let MetaData::Store(store_addr) = rob_entry.meta {
                        if store_addr == entry.addr {
                            return true;
                        }
                    }
                    false
                }) {
                    continue;
                }

                if let Some(result) = func_units.execute_load(&entry, mem) {
                    entry.status = LoadBufferStatus::Finished;
                    entry.value = result;
                } else if let LoadBufferStatus::Wait = entry.status {
                    entry.status = LoadBufferStatus::Execute;
                }
            }
        }
    }
}
