use super::operand::Operand;
use super::reorder_buffer::ReorderBuffer;
use super::reservation_staion::FinishedCalc;
use instruction::Opcode;
use memory::ProcessMemory;
use pipeline::functional_units::memory::MemoryUnit;
use register::RegisterFile;
use std::collections::HashMap;

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
    pub base: Operand,
    pub addr: u32, // initially has imm value.
    pub value: u32,
}

impl LoadBufferEntry {
    pub fn can_run(&self) -> bool {
        if let Operand::Value(_) = self.base {
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Default)]
pub struct LoadBuffer {
    buf: HashMap<usize, LoadBufferEntry>,
}

impl LoadBuffer {
    pub fn clear(&mut self) {
        self.buf.clear();
    }

    pub fn issue(&mut self, rob_index: usize, rob: &ReorderBuffer, reg: &RegisterFile) {
        let rob_entry = rob.get(rob_index).unwrap();
        let inst = &rob_entry.inst;
        match inst.opcode {
            Opcode::Load | Opcode::Amo => {
                self.buf.insert(
                    rob_index,
                    LoadBufferEntry {
                        rob_index,
                        status: LoadBufferStatus::Wait,
                        base: reg.get_reg_value(inst.fields.rs1.unwrap()),
                        addr: inst.fields.imm.unwrap_or(0),
                        value: 0,
                    },
                );
            }
            _ => unreachable!(),
        }
    }

    pub fn pop_finished(&mut self) -> Vec<FinishedCalc> {
        let finished: Vec<_> = self
            .buf
            .iter()
            .filter_map(|(idx, entry)| {
                if let LoadBufferStatus::Finished = entry.status {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect();
        finished
            .into_iter()
            .map(|idx| self.buf.remove(idx).unwrap())
            .map(|entry| FinishedCalc {
                rob_idx: entry.rob_index,
                reg_value: entry.value,
            })
            .collect()
    }

    pub fn propagate(&mut self, job: &FinishedCalc) {
        // Amo 연산들은 rs2도 전파되어야 retire
        for (load_idx, entry) in self.buf.iter_mut() {
            if let Operand::Rob(index) = entry.base {
                if index == job.rob_idx {
                    entry.base = Operand::Value(job.reg_value);
                    entry.addr += job.reg_value;
                }
            }
        }
    }

    fn is_load_ready(load: &LoadBufferEntry, rob: &ReorderBuffer) -> bool {
        use instruction::Function;
        if let LoadBufferStatus::Finished = load.status {
            return false;
        }
        if let Operand::Rob(_) = load.base {
            return false;
        }

        let rob_entry = rob.get(load.rob_index).unwrap();
        if let Opcode::Amo = rob_entry.inst.opcode {
            if let Operand::Rob(_) = rob_entry.mem_value {
                return false;
            }
        }

        let has_to_wait = rob.iter()
            .take(rob.to_relative_pos(load.rob_index).unwrap())
            .any(|entry| match entry.inst.opcode {
                Opcode::Store | Opcode::Amo if entry.inst.function != Function::Lrw => {
                    match entry.addr {
                        Operand::Rob(_) => true,
                        Operand::Value(addr) if addr == load.addr => true,
                        _ => false,
                    }
                }
                _ => false,
            });

        !has_to_wait
    }

    pub fn execute(&mut self, rob: &mut ReorderBuffer, mem: &ProcessMemory) {
        for (idx, entry) in self.buf.iter_mut() {
            if !Self::is_load_ready(entry, rob) {
                continue;
            }
            entry.status = LoadBufferStatus::Finished;

            let rob_entry = rob.get_mut(*idx).unwrap();
            rob_entry.mem_rem_cycle -= 1;
            if rob_entry.mem_rem_cycle == 0 {
                entry.value = MemoryUnit::execute(entry.addr, rob_entry, mem);
                entry.status = LoadBufferStatus::Finished;
            }
        }
    }
}
