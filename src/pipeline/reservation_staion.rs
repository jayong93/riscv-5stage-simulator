use super::functional_units as fu;
use super::load_buffer::LoadBuffer;
use super::operand::Operand;
use super::reorder_buffer::ReorderBuffer;
use instruction::{Function, Instruction, Opcode};
use memory::ProcessMemory;
use register::RegisterFile;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum RSStatus {
    Wait,
    Execute,
    Finished,
}

#[derive(Debug, Clone)]
pub struct RSEntry {
    pub rob_index: usize,
    pub status: RSStatus,
    pub inst: Instruction,
    pub operand: (Operand, Operand),
    pub value: u32,
    pub remaining_clock: usize,
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
    address_unit: fu::address::AddressUnit,
    load_buf: LoadBuffer,
    station: HashMap<usize, RSEntry>,
}

impl ReservationStation {
    pub fn clear(&mut self) {
        self.station.clear();
    }

    pub fn issue(&mut self, rob_index: usize, rob: &ReorderBuffer, reg: &RegisterFile) {
        let rob_entry = rob.get(rob_index).unwrap();
        let inst = &rob_entry.inst;
        match inst.opcode {
            Opcode::Store => self.address_unit.issue(rob_index, inst.clone(), reg),
            Opcode::Load => {
                self.address_unit.issue(rob_index, inst.clone(), reg);
                self.load_buf.issue(rob_index, rob, reg);
            }
            Opcode::Amo if inst.function != Function::Scw => {
                self.load_buf.issue(rob_index, rob, reg);
            }
            Opcode::Jalr => self.address_unit.issue(rob_index, inst.clone(), reg),
            Opcode::AuiPc | Opcode::Lui | Opcode::Jal | Opcode::Amo => {}
            _ => {
                let operand = {
                    let op1 = inst.fields.rs1.unwrap_or(0);
                    let op1 = reg.get_reg_value(op1);
                    let op2 = inst
                        .fields
                        .rs2
                        .map(|r| reg.get_reg_value(r))
                        .unwrap_or(Operand::Value(inst.fields.imm.unwrap_or(0)));
                    (op1, op2)
                };

                self.station.insert(
                    rob_index,
                    RSEntry {
                        rob_index,
                        status: RSStatus::Wait,
                        inst: inst.clone(),
                        operand,
                        value: 0,
                        remaining_clock: Self::remain_clock(inst.function),
                    },
                );
            }
        }
    }

    pub fn propagate(&mut self, job: &FinishedCalc) {
        self.address_unit.propagate(job);
        self.load_buf.propagate(job);

        for entry in self.station.values_mut() {
            let (op1, op2) = entry.operand;
            let ops = [op1, op2];
            let mut new_op_it = ops.iter().map(|op| match op {
                Operand::Rob(target_rob) if *target_rob == job.rob_idx => {
                    Operand::Value(job.reg_value)
                }
                _ => *op,
            });
            let new_op1 = new_op_it.next().unwrap();
            let new_op2 = new_op_it.next().unwrap();
            entry.operand = (new_op1, new_op2);
        }
    }

    // Jalr이 AddressUnit에서 계산 끝난 경우 pc를 반환
    pub fn execute(&mut self, rob: &mut ReorderBuffer, mem: &mut ProcessMemory) -> Option<u32> {
        let npc = self.address_unit.execute(rob);
        self.load_buf.execute(rob, mem);

        // Store
        let head_entry = rob.to_index(0).and_then(|idx| rob.get_mut(idx));
        if let Some(head) = head_entry {
            match head.inst.opcode {
                Opcode::Store | Opcode::Amo if head.inst.function != Function::Lrw => {
                    super::functional_units::memory::MemoryUnit::execute_store(
                        head,
                        mem,
                    )
                }
                _ => {}
            }
        }

        // General
        for entry in self.station.values_mut() {
            if let RSStatus::Finished = entry.status {
                continue;
            }

            if let (Operand::Value(a), Operand::Value(b)) = entry.operand {
                if let RSStatus::Wait = entry.status {
                    entry.status = RSStatus::Execute
                }
                entry.remaining_clock -= 1;
                if entry.remaining_clock == 0 {
                    entry.value = crate::alu::alu(&entry.inst.function, a as i32, b as i32) as u32;
                    entry.status = RSStatus::Finished;
                }
            }
        }
        npc
    }

    pub fn completed_jobs(&mut self) -> Vec<FinishedCalc> {
        let mut loads = self.load_buf.pop_finished();
        let mut generals: Vec<_> = {
            let finished: Vec<_> = self
                .station
                .iter()
                .filter_map(|(&idx, entry)| {
                    if let RSStatus::Finished = entry.status {
                        Some(idx)
                    } else {
                        None
                    }
                })
                .collect();

            finished
                .into_iter()
                .map(|idx| self.station.remove(&idx).unwrap())
                .map(|entry| FinishedCalc {
                    rob_idx: entry.rob_index,
                    reg_value: entry.value,
                })
                .collect()
        };

        loads.append(&mut generals);
        loads
    }

    fn remain_clock(func: Function) -> usize {
        use self::Function::*;
        match func {
            Mul | Mulh | Mulhsu | Mulhu => 4,
            Div | Divu | Rem | Remu => 8,
            _ => 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FinishedCalc {
    pub rob_idx: usize,
    pub reg_value: u32,
}
