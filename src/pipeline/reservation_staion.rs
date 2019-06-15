use super::operand::Operand;
use super::reorder_buffer::{MetaData, ReorderBuffer};
use super::functional_units::FunctionalUnits;
use instruction::Instruction;
use register::RegisterFile;
use std::collections::{HashMap, VecDeque};

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

impl RSEntry {
    pub fn operand_values(&self) -> (Option<u32>, Option<u32>) {
        let (op1, op2) = self.operand;
        let op1 = match op1 {
            Operand::Rob(_) => None,
            Operand::Value(v) => Some(v),
        };
        let op2 = match op2 {
            Operand::Rob(_) => None,
            Operand::Value(v) => Some(v),
        };

        (op1, op2)
    }
}

#[derive(Debug, Default)]
pub struct ReservationStation {
    station: HashMap<usize, RSEntry>,
}

impl ReservationStation {
    pub fn clear(&mut self) {
        self.station.clear();
    }

    pub fn issue(&mut self, rob_index: usize, rob: &ReorderBuffer, reg: &RegisterFile) {
        let rob_entry = rob.get(rob_index).unwrap();
        let inst = rob_entry.inst.clone();
        let operands = [
            inst.fields.rs1,
            inst.fields.rs2,
        ];
        let mut operands: VecDeque<_> = operands
            .into_iter()
            .enumerate()
            .map(|(i, rs)| {
                if i == 1 && rs.is_none() {
                    return Operand::Value(inst.fields.imm.unwrap());
                }

                let rs = rs.unwrap_or(0) as usize;
                let val = reg.gpr[rs].read();
                rob.iter()
                    .enumerate()
                    .rev()
                    .find_map(|(rel_pos, entry)| {
                        if let MetaData::Normal(reg) = entry.meta {
                            if reg as usize == rs {
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
        self.station.insert(rob_index, new_entry);
    }

    pub fn pop_finished(&mut self) -> Vec<RSEntry> {
        let mut finished = Vec::new();
        let finished_indices: Vec<_> = self
            .station
            .iter()
            .filter(|(_, entry)| {
                if let RSStatus::Finished = entry.status {
                    true
                } else {
                    false
                }
            })
            .map(|(idx, _)| *idx)
            .collect();

        for idx in finished_indices {
            finished.push(self.station.remove(&idx).unwrap());
        }
        finished
    }

    pub fn propagate(&mut self, rob_index: usize, value: u32) {
        for entry in self.station.values_mut() {
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
        for entry in self.station.values_mut() {
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
