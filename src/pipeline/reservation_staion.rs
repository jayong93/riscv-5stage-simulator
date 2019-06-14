use super::operand::Operand;
use super::reorder_buffer::{MetaData, ReorderBuffer};
use instruction::Instruction;
use register::RegisterFile;
use std::collections::{LinkedList, VecDeque};

#[derive(Debug)]
pub enum RSStatus {
    Wait,
    Execute,
    Finished,
}

#[derive(Debug)]
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
        
        unimplemented!()
    }
}
