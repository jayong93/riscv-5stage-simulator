pub mod iter;
use super::operand::Operand;
use instruction::{Function, Instruction, Opcode};
use memory::ProcessMemory;
use pipeline::branch_predictor::BranchPredictor;
use pipeline::reservation_staion::FinishedCalc;
use pipeline::Pipeline;
use register::RegisterFile;
use std::collections::{HashMap, VecDeque};
use std::fmt::Debug;
use pipeline::exception::Exception;


#[derive(Debug, Clone)]
pub struct ReorderBufferEntry {
    pub pc: u32,
    pub inst: Instruction,
    pub mem_value: Operand,
    pub reg_value: Option<u32>,
    pub rd: u8,
    pub addr: Operand,
    pub branch_pred: bool,
    pub mem_rem_cycle: usize,
    pub mem_exception: Result<(), Exception>,
}

impl ReorderBufferEntry {
    pub fn is_completed(&self) -> bool {
        let mem_val_done = if let Operand::Value(_) = self.mem_value {
            true
        } else {
            false
        };
        let addr_done = if let Operand::Value(_) = self.addr {
            true
        } else {
            false
        };
        let reg_val_done = self.reg_value.is_some();

        match self.inst.opcode {
            Opcode::Store => mem_val_done && addr_done && self.mem_rem_cycle == 0,
            Opcode::Amo => mem_val_done && addr_done && reg_val_done && self.mem_rem_cycle == 0,
            Opcode::Jalr => addr_done && reg_val_done,
            _ => reg_val_done,
        }
    }

    // true 반환이면 branch prediction miss
    pub fn retire(
        &self,
        old_index: usize,
        memory: &mut ProcessMemory,
        reg: &mut RegisterFile,
    ) -> bool {
        self.mem_exception.unwrap();

        if let Opcode::Branch = self.inst.opcode {
            if let Some(branch_result) = self.reg_value {
                if branch_result == self.branch_pred as u32 {
                    return false;
                }
            }
            return true;
        }

        if let Function::Ecall = self.inst.function {
            Pipeline::system_call(memory, reg).unwrap();
            return false;
        }

        if let Some(reg_val) = self.reg_value {
            reg.gpr[self.rd as usize].write(reg_val);
            if let Some(related_rob) = reg.related_rob[self.rd as usize] {
                if related_rob == old_index {
                    reg.related_rob[self.rd as usize] = None;
                }
            }
        }

        false
    }
}

#[derive(Debug, Default)]
pub struct ReorderBuffer {
    highst_index: usize,
    unused_indies: VecDeque<usize>,
    index_queue: VecDeque<usize>,
    index_map: HashMap<usize, usize>,
    buf: Vec<(usize, ReorderBufferEntry)>,
}

impl std::fmt::Display for ReorderBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "[\n")?;
        self.iter_with_id().for_each(|entry| {
            write!(f, "{:?}\n", entry).unwrap();
        });
        write!(f, "]")
    }
}

impl ReorderBuffer {
    pub fn clear(&mut self) {
        self.highst_index = 0;
        self.unused_indies.clear();
        self.index_map.clear();
        self.index_queue.clear();
        self.buf.clear();
    }

    fn register_new_index(&mut self) -> usize {
        self.unused_indies.pop_front().unwrap_or_else(|| {
            let new_index = self.highst_index;
            self.highst_index += 1;
            new_index
        })
    }

    fn add(&mut self, entry: ReorderBufferEntry) -> usize {
        let index = self.register_new_index();
        self.index_queue.push_back(index);
        self.buf.push((index, entry));
        self.index_map.insert(index, self.buf.len() - 1);
        index
    }

    pub fn pop_front(&mut self) -> Option<ReorderBufferEntry> {
        if self.buf.is_empty() {
            return None;
        }

        let index = self.index_queue.pop_front().unwrap();
        if let Some(raw_idx_to_remove) = self.index_map.remove(&index) {
            let last_idx;
            {
                let (idx, _) = self.buf.last().unwrap();
                last_idx = *idx;
            }
            let (_, removed_entry) = self.buf.swap_remove(raw_idx_to_remove);
            if let Some(last_raw_idx) = self.index_map.get_mut(&last_idx) {
                *last_raw_idx = raw_idx_to_remove;
            }
            self.unused_indies.push_back(index);

            Some(removed_entry)
        } else {
            None
        }
    }

    pub fn issue(
        &mut self,
        pc: u32,
        inst: Instruction,
        reg: &crate::register::RegisterFile,
        branch_predictor: &mut BranchPredictor,
    ) -> usize {
        let (mem_value, addr) = match inst.opcode {
            Opcode::Store => (
                reg.get_reg_value(inst.fields.rs2.unwrap(), self),
                Operand::default(),
            ),
            Opcode::Amo => (
                reg.get_reg_value(inst.fields.rs2.unwrap(), self),
                reg.get_reg_value(inst.fields.rs1.unwrap(), self),
            ),
            _ => (Operand::default(), Operand::default()),
        };
        let rd = inst.fields.rd.unwrap_or(0);
        let branch_pred = if let Opcode::Branch = inst.opcode {
            branch_predictor.predict(pc)
        } else {
            false
        };
        let new_entry = ReorderBufferEntry {
            pc,
            inst,
            reg_value: None,
            mem_value,
            rd,
            addr,
            branch_pred,
            mem_rem_cycle: crate::consts::MEM_CYCLE,
            mem_exception: Ok(()),
        };

        self.add(new_entry)
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn get(&self, index: usize) -> Option<&ReorderBufferEntry> {
        self.index_map
            .get(&index)
            .and_then(|&raw_idx| self.buf.get(raw_idx))
            .map(|(_, entry)| entry)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut ReorderBufferEntry> {
        if let Some(raw_idx) = self.index_map.get(&index) {
            self.buf.get_mut(*raw_idx).map(|(_, entry)| entry)
        } else {
            None
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &ReorderBufferEntry> + DoubleEndedIterator + Debug {
        self.iter_with_id().map(|pair| &pair.1)
    }

    pub fn nth_index(&self, n: usize) -> Option<usize> {
        self.index_queue.get(n).map(|idx| *idx)
    }

    pub fn iter_with_id(
        &self,
    ) -> impl Iterator<Item = &(usize, ReorderBufferEntry)> + DoubleEndedIterator + Debug {
        iter::Iter {
            index_map: &self.index_map,
            index_queue: &self.index_queue,
            buf: &self.buf,
            cur_head: 0,
            cur_tail: self.index_queue.len(),
        }
    }

    pub fn propagate(&mut self, job: &FinishedCalc) {
        for (idx, entry) in self.buf.iter_mut() {
            if *idx == job.rob_idx {
                entry.reg_value = Some(job.reg_value);
                if let Some(exception) = job.exception {
                    entry.mem_exception = Err(exception);
                }
                continue;
            }

            let ops = [entry.addr, entry.mem_value];
            let mut it = ops.into_iter().map(|op| match op {
                Operand::Rob(target_rob) if *target_rob == job.rob_idx => {
                    Operand::Value(job.reg_value)
                }
                _ => *op,
            });
            entry.addr = it.next().unwrap();
            entry.mem_value = it.next().unwrap();
        }
    }

    pub fn completed_entries(&mut self) -> Vec<(usize, ReorderBufferEntry)> {
        let completed: Vec<_> = self
            .iter_with_id()
            .take_while(|(_, entry)| entry.is_completed())
            .map(|(idx, _)| *idx)
            .collect();

        completed
            .into_iter()
            .map(|idx| (idx, self.pop_front().unwrap()))
            .collect()
    }
}
