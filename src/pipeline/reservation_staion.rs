
use super::reorder_buffer::ReorderBuffer;
use instruction::Instruction;
use std::collection::LinkedList;

#[derive(Debug)]
pub enum RSStatus {
    Wait,
    Execute,
}

#[derive(Debug)]
pub enum RSOperand {
    Value(u32),
    Rob(usize),
}

#[derive(Debug)]
pub struct RSEntry {
    rob_index: usize,
    status: RSStatus,
    inst: Instruction,
    operand: (RSOperand, RSOperand),
}

#[derive(Debug, Default)]
pub struct ReservationStation {
    station: LinkedList<RSEntry>
}

impl ReservationStation {
    pub fn clear(&mut self) {
        self.station.clear();
    }

    pub fn issue(&mut self, rob_index: usize, rob: &ReorderBuffer) {
        let new_entry = RSEntry {
            rob_index,
            status: RSStatus::Wait,
            inst: rob.get(rob_index).unwrap(),
            operand
        };
        self.station.push_back()
    }
}
