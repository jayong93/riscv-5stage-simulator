use super::operand::Operand;
use super::reorder_buffer::ReorderBuffer;
use super::reservation_staion::FinishedCalc;
use instruction::Opcode;
use memory::ProcessMemory;
use pipeline::exception::Exception;
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
    pub value: Result<u32, Exception>,
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
                        value: Ok(0),
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
            .filter_map(|(&idx, entry)| {
                if let LoadBufferStatus::Finished = entry.status {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect();
        finished
            .into_iter()
            .map(|idx| self.buf.remove(&idx).unwrap())
            .map(|entry| FinishedCalc {
                rob_idx: entry.rob_index,
                reg_value: entry.value.unwrap_or(0),
                exception: entry.value.err(),
            })
            .collect()
    }

    fn is_load_ready(load: &LoadBufferEntry, rob: &ReorderBuffer) -> bool {
        use instruction::Function;
        if let LoadBufferStatus::Finished = load.status {
            return false;
        }

        let rob_entry = rob.get(load.rob_index).unwrap();
        let mut my_addr = 0;
        if let Operand::Value(target_addr) = rob_entry.addr {
            my_addr = target_addr;
        } else {
            return false;
        }

        // Amo는 RS2까지 대기하다가 실행
        if let Opcode::Amo = rob_entry.inst.opcode {
            if let Operand::Rob(_) = rob_entry.mem_value {
                return false;
            }
        }

        let has_to_wait = rob
            .iter_with_id()
            .take_while(|(id, _)| *id != load.rob_index)
            .any(|(_, entry)| match entry.inst.opcode {
                Opcode::Store | Opcode::Amo if entry.inst.function != Function::Lrw => {
                    match entry.addr {
                        Operand::Rob(_) => true,
                        Operand::Value(addr) if addr == my_addr => true,
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
            entry.status = LoadBufferStatus::Execute;

            let rob_entry = rob.get_mut(*idx).unwrap();
            let addr = if let Operand::Value(a) = rob_entry.addr {
                a
            } else {
                unreachable!()
            };

            rob_entry.mem_rem_cycle = rob_entry.mem_rem_cycle.saturating_sub(1);
            if rob_entry.mem_rem_cycle == 0 {
                entry.value = MemoryUnit::execute(addr, rob_entry, mem);
                entry.status = LoadBufferStatus::Finished;
            }
        }
    }
}
