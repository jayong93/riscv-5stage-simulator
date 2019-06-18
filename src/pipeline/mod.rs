//! Pipeline definition.

pub mod branch_predictor;
pub mod exception;
pub mod functional_units;
pub mod load_buffer;
pub mod operand;
pub mod reorder_buffer;
pub mod reservation_staion;

use self::exception::Exception;
use self::reorder_buffer::ReorderBufferEntry;
use consts;
use instruction::Function;
use memory;
use register;

/// Pipeline holding four inter-stage registers
#[derive(Debug)]
pub struct Pipeline {
    pub reg: register::RegisterFile,
    pub memory: memory::ProcessMemory,
    pub rob: reorder_buffer::ReorderBuffer,
    pub rs: reservation_staion::ReservationStation,
    pub branch_predictor: branch_predictor::BranchPredictor,
    pub clock: usize,
}

impl Pipeline {
    pub fn new(entry_point: u32, memory: memory::ProcessMemory) -> Pipeline {
        Pipeline {
            reg: register::RegisterFile::new(entry_point, memory.stack_pointer_init),
            memory,
            rob: Default::default(),
            rs: Default::default(),
            branch_predictor: Default::default(),
            clock: 0,
        }
    }

    fn clear_all_buffers(&mut self) {
        self.rs.clear();
        self.rob.clear();
        self.reg
            .related_rob
            .iter_mut()
            .for_each(|stat| *stat = None);
    }

    pub fn system_call(
        memory: &mut memory::ProcessMemory,
        reg: &mut register::RegisterFile,
    ) -> Result<(), Exception> {
        let syscall_num = reg.gpr[consts::SYSCALL_NUM_REG].read();
        let calling_exception = |_| Exception::FailCallingSyscall(syscall_num);
        let result: Result<u32, Exception> = match syscall_num {
            64 => {
                let fd = reg.gpr[consts::SYSCALL_ARG1_REG].read() as i32;
                let buf_addr = reg.gpr[consts::SYSCALL_ARG2_REG].read();
                let count = reg.gpr[consts::SYSCALL_ARG3_REG].read();
                memory
                    .read_bytes(buf_addr, count as usize)
                    .and_then(|bytes| {
                        nix::unistd::write(fd, bytes)
                            .map(|n| n as u32)
                            .map_err(calling_exception)
                    })
            }
            78 => {
                let buf_addr = reg.gpr[consts::SYSCALL_ARG3_REG].read();
                let buf_size = reg.gpr[consts::SYSCALL_ARG4_REG].read();
                let path_addr = reg.gpr[consts::SYSCALL_ARG2_REG].read();
                let fd = reg.gpr[consts::SYSCALL_ARG1_REG].read();

                let path_addr = memory.read_bytes(path_addr, 1).unwrap().as_ptr() as *const i8;
                let path_str = unsafe { std::ffi::CStr::from_ptr(path_addr) }
                    .to_str()
                    .expect("Can't convert bytes to str");
                memory
                    .read_bytes_mut(buf_addr, buf_size as usize)
                    .and_then(|buf| {
                        nix::fcntl::readlinkat(fd as i32, path_str, buf)
                            .map(|s| s.len() as u32)
                            .map_err(calling_exception)
                    })
            }
            80 => {
                let fd = reg.gpr[consts::SYSCALL_ARG1_REG].read();
                let buf_addr = reg.gpr[consts::SYSCALL_ARG2_REG].read();
                let stat = nix::sys::stat::fstat(fd as i32).unwrap();

                memory.write(buf_addr, stat).map(|_| 0)
            }
            160 => {
                let addr = reg.gpr[consts::SYSCALL_ARG1_REG].read();
                memory.write(addr, nix::sys::utsname::uname()).map(|_| 0)
            }
            174 => Ok(nix::unistd::getuid().as_raw()),
            175 => Ok(nix::unistd::geteuid().as_raw()),
            176 => Ok(nix::unistd::getgid().as_raw()),
            177 => Ok(nix::unistd::getegid().as_raw()),
            214 => {
                let addr = reg.gpr[consts::SYSCALL_ARG1_REG].read();
                let max_mem_addr = memory.v_address_range.1;
                if max_mem_addr <= addr {
                    memory.data.resize((addr - max_mem_addr + 1) as usize, 0);
                    memory.v_address_range.1 = addr + 1;
                }
                Ok(addr)
            }
            93 | 94 => Ok(0),
            _ => Err(Exception::FailCallingSyscall(syscall_num)),
        };
        result.map(|ret_val| {
            reg.gpr[consts::SYSCALL_RET_REG].write(ret_val);
            ()
        })
    }

    pub fn commit(&mut self) -> Vec<(usize, ReorderBufferEntry)> {
        use instruction::Opcode;
        let mut completed_entries = self.rob.completed_entries();
        let retired_count = completed_entries
            .iter()
            .map(|(old_idx, entry)| {
                let should_cancel = entry.retire(*old_idx, &mut self.memory, &mut self.reg);

                if let Opcode::Branch = entry.inst.opcode {
                    self.branch_predictor
                        .update(entry.pc, entry.reg_value.unwrap());
                }

                if unsafe { crate::PRINT_STEPS } {
                    eprint!(
                        "Clock #{} | pc: {:x} | val: {:08x} | inst: {:?} | fields: {}",
                        self.clock,
                        entry.pc,
                        entry.inst.value,
                        entry.inst.function,
                        entry.inst.fields,
                    );
                    if unsafe { crate::PRINT_DEBUG_INFO } {
                        eprint!(" | regs: {}", self.reg);
                    }
                    eprintln!("");
                }

                if should_cancel {
                    self.clear_all_buffers();
                    if let (Opcode::Branch, Some(is_taken)) = (entry.inst.opcode, entry.reg_value) {
                        if is_taken == 1 {
                            self.reg
                                .pc
                                .write(entry.pc.wrapping_add(entry.inst.fields.imm.unwrap()));
                        } else {
                            self.reg
                                .pc
                                .write(entry.pc.wrapping_add(crate::consts::WORD_SIZE as u32));
                        }
                    }
                }
                should_cancel
            })
            .take_while(|&should_cancel| !should_cancel)
            .count();
        let total_len = completed_entries.len();
        completed_entries.truncate(std::cmp::min(retired_count + 1, total_len));
        completed_entries
    }

    fn is_program_finished(&self, retired_entries: &[(usize, ReorderBufferEntry)]) -> bool {
        retired_entries.into_iter().any(|(_, rob_entry)| {
            if let Function::Ecall = rob_entry.inst.function {
                if let 93 | 94 = self.reg.gpr[consts::SYSCALL_NUM_REG].read() {
                    return true;
                }
            }
            false
        })
    }

    pub fn write_result(&mut self) {
        let completed_entries = self.rs.completed_jobs();
        for entry in completed_entries {
            self.rs.propagate(&entry);
            self.rob.propagate(&entry);
        }
    }

    pub fn execute(&mut self) {
        let npc = self.rs.execute(&mut self.rob, &mut self.memory);
        if let Some(npc) = npc {
            self.reg.pc.write(npc);
        }
    }

    pub fn issue(&mut self) {
        use instruction::Function::*;
        use instruction::{Instruction, Opcode};

        // stall
        {
            let last_rob_entry = self.rob.iter().rev().next();
            if let Some(entry) = last_rob_entry {
                let has_to_stall = match entry.inst.function {
                    Ecall => true,
                    Jalr if !entry.is_completed() => true,
                    _ => false,
                };
                if has_to_stall {
                    return;
                }
            }
        }

        for _ in 0..2 {
            let pc = self.reg.pc.read();
            let raw_inst = self.memory.read_inst(pc).unwrap();
            let mut inst = Instruction::new(raw_inst);
            if let Opcode::Fmadd
            | Opcode::Fmsub
            | Opcode::Fnmadd
            | Opcode::Fnmsub
            | Opcode::OpFp
            | Opcode::StoreFp
            | Opcode::LoadFp = inst.opcode
            {
                inst = Instruction::default();
            }

            let (npc, has_to_stop) = match inst.opcode {
                Opcode::Jal => (pc.wrapping_add(inst.fields.imm.unwrap()), true),
                Opcode::Jalr => (pc, true),
                Opcode::System if inst.function == Function::Ecall => {
                    (pc.wrapping_add(consts::WORD_SIZE as u32), true)
                }
                Opcode::Branch => {
                    let npc = if self.branch_predictor.predict(pc) {
                        // taken
                        pc.wrapping_add(inst.fields.imm.unwrap())
                    } else {
                        pc.wrapping_add(consts::WORD_SIZE as u32)
                    };
                    (npc, false)
                }
                _ => (pc.wrapping_add(consts::WORD_SIZE as u32), false),
            };
            self.reg.pc.write(npc);

            let inst_rd = inst.fields.rd.unwrap_or(0);
            let rob_idx = self.rob.issue(pc, inst, &self.reg, &mut self.branch_predictor);
            self.rs.issue(rob_idx, &self.rob, &self.reg);
            self.reg.set_reg_rob_index(inst_rd, rob_idx);

            if has_to_stop {
                break;
            }
        }
    }
    // return true when process ends.
    pub fn run_clock(&mut self) -> (Vec<(usize, ReorderBufferEntry)>, bool) {
        self.clock += 1;
        let retired_insts = self.commit();
        if self.is_program_finished(&retired_insts) {
            return (retired_insts, true);
        }

        self.write_result();
        self.execute();
        self.issue();
        (retired_insts, false)
    }
}
