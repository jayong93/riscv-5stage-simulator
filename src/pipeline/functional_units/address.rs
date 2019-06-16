use instruction::{Instruction, Opcode};
use pipeline::operand::Operand;
use pipeline::reorder_buffer::ReorderBuffer;
use pipeline::reservation_staion::{FinishedCalc, RSEntry, RSStatus};
use register::RegisterFile;
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct AddressUnit {
    pub buf: HashMap<usize, RSEntry>,
}

/**
 * Store, Load, Jalr의 주소 계산
 * Store와 Load는 Rob entry만 갱신하면 되고
 * Jalr은 거기에 PC까지 같이 갱신해야 함.
 */
impl AddressUnit {
    pub fn clear(&mut self) {
        self.buf.clear()
    }
    pub fn issue(&mut self, rob_idx: usize, inst: Instruction, reg: &RegisterFile, rob: &ReorderBuffer) {
        let rs1 = inst.fields.rs1.unwrap();
        let imm = inst.fields.imm.unwrap_or(0);
        let entry = RSEntry {
            rob_index: rob_idx,
            status: RSStatus::Wait,
            inst,
            operand: (reg.get_reg_value(rs1, rob), Operand::Value(imm)),
            value: 0,
            remaining_clock: 1,
        };
        {
            if rob.get(rob_idx).unwrap().pc == 0x295ec {
                eprintln!("{:?}", entry);
            }
        }
        self.buf.insert(rob_idx, entry);
    }

    pub fn propagate(&mut self, job: &FinishedCalc) {
        for entry in self.buf.values_mut() {
            let (op1, op2) = entry.operand;
            let new_op1 = match op1 {
                Operand::Rob(target_rob) if target_rob == job.rob_idx => {
                    Operand::Value(job.reg_value)
                }
                _ => op1,
            };
            entry.operand = (new_op1, op2);
        }
    }

    pub fn execute(&mut self, rob: &mut ReorderBuffer) -> Option<u32> {
        // Jalr의 실행이 끝난경우 npc 반환
        let (finished_idx, npc) = self
            .buf
            .iter_mut()
            .filter_map(|(rob_idx, entry)| {
                if let (Operand::Value(reg_val), Operand::Value(imm)) = entry.operand {
                    entry.value = reg_val.wrapping_add(imm);
                    entry.status = RSStatus::Finished;
                    let rob_entry = rob.get_mut(*rob_idx).unwrap();
                    if rob_entry.pc == 0x295ec {
                        eprintln!("{:?}", entry);
                    }
                    rob_entry.addr = Operand::Value(entry.value);
                    Some((
                        rob_idx,
                        if entry.inst.opcode == Opcode::Jalr {
                            entry.value
                        } else {
                            0
                        },
                    ))
                } else {
                    None
                }
            })
            .fold((Vec::new(), 0u32), |(mut vec, npc), (&idx, jalr_val)| {
                vec.push(idx);
                if npc == 0 {
                    (vec, jalr_val)
                } else {
                    (vec, npc)
                }
            });

        for idx in finished_idx {
            self.buf.remove(&idx);
        }

        if npc == 0 {
            None
        } else {
            Some(npc)
        }
    }
}
