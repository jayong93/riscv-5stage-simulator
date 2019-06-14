use super::operand::Operand;
use super::reorder_buffer::{MetaData, ReorderBuffer};
use super::functional_units::FunctionalUnits;
use instruction::Instruction;
use register::RegisterFile;
use std::collections::{LinkedList, VecDeque};

#[derive(Debug, Clone)]
pub enum RSStatus {
    Wait,
    Execute,
    Finished,
}

#[derive(Debug, Clone)]
pub struct RSEntry {
    pub rob_index: usize,
    status: RSStatus,
    pub inst: Instruction,
    operand: (Operand, Operand),
    pub value: u32,
}

#[derive(Debug, Default)]
pub struct ReservationStation {
    station: LinkedList<RSEntry>,
}

impl ReservationStation {
    pub fn clear(&mut self) {
        self.station.clear();
    }

    pub fn issue(&mut self, rob_index: usize, rob: &ReorderBuffer, reg: &RegisterFile) {
        let rob_entry = rob.get(rob_index).unwrap();
        let inst = rob_entry.inst.clone();
        let operands = [
            inst.fields.rs1.unwrap_or(0) as usize,
            inst.fields.rs2.unwrap_or(0) as usize,
        ];
        let mut operands: VecDeque<_> = operands
            .into_iter()
            .map(|rs| {
                let val = reg.gpr[*rs].read();
                rob.iter()
                    .enumerate()
                    .rev()
                    .find_map(|(rel_pos, entry)| {
                        if let MetaData::Normal(reg) = entry.meta {
                            if reg as usize == *rs {
                                return rob.to_index(rel_pos);
                            }
                        }
                        None
                    })
                    .map_or(Operand::Value(val), |idx| Operand::Rob(idx))
            })
            .collect();

        let new_entry = RSEntry {
            rob_index,
            status: if rob_entry.is_ready {
                RSStatus::Finished
            } else {
                RSStatus::Wait
            },
            inst,
            operand: (operands.pop_front().unwrap(), operands.pop_front().unwrap()),
            value: 0,
        };
        self.station.push_back(new_entry);
    }

    pub fn pop_finished(&mut self) -> Vec<RSEntry> {
        let finished_num = self
            .station
            .iter()
            .filter(|entry| {
                if let RSStatus::Finished = entry.status {
                    true
                } else {
                    false
                }
            })
            .count();

        let mut finished = Vec::new();
        for _ in 0..finished_num {
            finished.push(self.station.pop_front().unwrap());
        }
        finished
    }

    pub fn propagate(&mut self, rob_index: usize, value: u32) {
        for entry in self.station.iter_mut() {
            let (op1, op2) = entry.operand;
            if let Operand::Rob(index) = op1 {
                if index == rob_index {
                    entry.operand.0 = Operand::Value(value);
                }
            }
            if let Operand::Rob(index) = op2 {
                if index == rob_index {
                    entry.operand.1 = Operand::Value(value);
                }
            }
        }
    }

    pub fn execute(&mut self, rob: &ReorderBuffer, func_units: &mut FunctionalUnits) {
        for entry in self.station.iter_mut() {
            match entry.status {
                RSStatus::Wait => {
                    match entry.operand {
                        (Operand::Value(_), Operand::Value(_)) => {
                            if let Some(result) = func_units.execute_general(&entry) {
                                entry.status = RSStatus::Finished;
                                entry.value = result;
                            } else {
                                entry.status = RSStatus::Execute;
                            }
                        }
                        _ => {},
                    }
                },
                RSStatus::Execute => {
                    if let Some(result) = func_units.execute_general(&entry) {
                        entry.status = RSStatus::Finished;
                        entry.value = result;
                    }
                },
                _ => {}
            }
        }
    }
}
