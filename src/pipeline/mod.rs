//! Pipeline definition.

pub mod branch_predictor;
pub mod load_buffer;
pub mod operand;
pub mod reorder_buffer;
pub mod reservation_staion;

use self::reorder_buffer::ReorderBufferEntry;
use consts;
use instruction::{Function, Instruction, Opcode};
use memory;
use register;

enum SyscallError {
    NotImpl,
    Error(String),
}

/// Pipeline holding four inter-stage registers
#[derive(Debug)]
pub struct Pipeline {
    pub reg: register::RegisterFile,
    pub memory: memory::ProcessMemory,
    pub rob: reorder_buffer::ReorderBuffer,
    pub rs: reservation_staion::ReservationStation,
    pub lb: load_buffer::LoadBuffer,
}

impl Pipeline {
    pub fn new(entry_point: u32, memory: memory::ProcessMemory) -> Pipeline {
        Pipeline {
            reg: register::RegisterFile::new(entry_point, memory.stack_pointer_init),
            memory,
            rob: Default::default(),
            rs: Default::default(),
            lb: Default::default(),
        }
    }

    fn clear_all_buffers(&mut self) {
        self.rs.clear();
        self.lb.clear();
        self.rob.clear();
    }

    fn system_call(&mut self) -> Result<(), SyscallError> {
        let result: Result<u32, SyscallError> = match self.reg.gpr[consts::SYSCALL_NUM_REG].read() {
            64 => {
                let fd = self.reg.gpr[consts::SYSCALL_ARG1_REG].read() as i32;
                let buf_addr = self.reg.gpr[consts::SYSCALL_ARG2_REG].read();
                let count = self.reg.gpr[consts::SYSCALL_ARG3_REG].read();
                let bytes = self.memory.read_bytes(buf_addr, count as usize);

                nix::unistd::write(fd, bytes)
                    .map(|n| n as u32)
                    .map_err(|err| SyscallError::Error(format!("{}", err)))
            }
            78 => {
                let buf_addr = self.reg.gpr[consts::SYSCALL_ARG3_REG].read();
                let buf_size = self.reg.gpr[consts::SYSCALL_ARG4_REG].read();
                let path_addr = self.reg.gpr[consts::SYSCALL_ARG2_REG].read();
                let fd = self.reg.gpr[consts::SYSCALL_ARG1_REG].read();

                let path_addr = self.memory.read_bytes(path_addr, 1).as_ptr() as *const i8;
                let path_str = unsafe { std::ffi::CStr::from_ptr(path_addr) }
                    .to_str()
                    .expect("Can't convert bytes to str");
                let buf = self.memory.read_bytes_mut(buf_addr, buf_size as usize);
                nix::fcntl::readlinkat(fd as i32, path_str, buf)
                    .map(|s| s.len() as u32)
                    .map_err(|_| {
                        SyscallError::Error("Can't call readlinkat system call".to_string())
                    })
            }
            80 => {
                let fd = self.reg.gpr[consts::SYSCALL_ARG1_REG].read();
                let buf_addr = self.reg.gpr[consts::SYSCALL_ARG2_REG].read();
                let stat = nix::sys::stat::fstat(fd as i32).unwrap();

                self.memory
                    .write(buf_addr, stat)
                    .map(|_| 0)
                    .map_err(|s| SyscallError::Error(s))
            }
            160 => {
                let addr = self.reg.gpr[consts::SYSCALL_ARG1_REG].read();
                self.memory
                    .write(addr, nix::sys::utsname::uname())
                    .map(|_| 0)
                    .map_err(|s| SyscallError::Error(s))
            }
            174 => Ok(nix::unistd::getuid().as_raw()),
            175 => Ok(nix::unistd::geteuid().as_raw()),
            176 => Ok(nix::unistd::getgid().as_raw()),
            177 => Ok(nix::unistd::getegid().as_raw()),
            214 => {
                let addr = self.reg.gpr[consts::SYSCALL_ARG1_REG].read();
                let max_mem_addr = self.memory.v_address_range.1;
                if max_mem_addr <= addr {
                    self.memory
                        .data
                        .resize((addr - max_mem_addr + 1) as usize, 0);
                    self.memory.v_address_range.1 = addr + 1;
                }
                Ok(addr)
            }
            _ => Err(SyscallError::NotImpl),
        };
        result.map(|ret_val| {
            self.reg.gpr[consts::SYSCALL_RET_REG].write(ret_val);
            ()
        })
    }

    pub fn commit(&mut self) -> Vec<ReorderBufferEntry> {
        use self::reorder_buffer::MetaData::*;
        let retired_entries = self.rob.retire();

        for entry in retired_entries.iter() {
            match entry.meta {
                Branch {
                    pred_taken: pred,
                    is_taken: real,
                } => {
                    if pred != real {
                        self.clear_all_buffers();
                        if real {
                            self.reg.pc.write(entry.value);
                        } else {
                            self.reg
                                .pc
                                .write(entry.pc + crate::consts::WORD_SIZE as u32);
                        }
                    }
                }
                Store(addr) => {
                    self.memory.write(addr, entry.value).unwrap();
                }
                Normal(reg) => {
                    self.reg.gpr[reg as usize].write(entry.value);
                }
                Syscall => {
                    self.system_call().unwrap_or_else(|e| {
                        if let SyscallError::Error(s) = e {
                            panic!("{}, in {}, registers: {}", s, entry.pc, self.reg)
                        } else {
                            panic!(
                                "(in {})system call #{} is not implemented yet",
                                entry.pc,
                                self.reg.gpr[consts::SYSCALL_NUM_REG].read()
                            )
                        }
                    });
                }
                _ => {}
            }
        }

        retired_entries
    }

    fn is_program_finished(&self, retired_entries: &[ReorderBufferEntry]) -> bool {
        use self::reorder_buffer::MetaData::*;
        retired_entries.into_iter().any(|rob_entry| {
            if let Syscall = rob_entry.meta {
                if let 93 | 94 = self.reg.gpr[consts::SYSCALL_NUM_REG].read() {
                    return true;
                }
            }
            false
        })
    }

    pub fn process_rs_entry(&mut self, entry: self::reservation_staion::RSEntry) {
        use self::reorder_buffer::MetaData;
        let rob_entry = self.rob.get_mut(entry.rob_index).unwrap();
        if entry.inst.function == Function::Jalr {
            self.reg.pc.write(entry.value);
        } else if let MetaData::Branch {
            pred_taken: pred,
            is_taken: _,
        } = rob_entry.meta
        {
            // branch의 결과값은 meta에 쓴다.
            rob_entry.meta = MetaData::Branch {
                is_taken: if entry.value == 0 {false} else {true},
                pred_taken: pred,
            };
        } else if !rob_entry.is_ready {
            self.lb.propagate(entry.rob_index, entry.value);
            self.rs.propagate(entry.rob_index, entry.value);
            rob_entry.value = entry.value;
        }
        rob_entry.is_ready = true;
    }

    pub fn process_lb_entry(&mut self, entry: self::load_buffer::LoadBufferEntry) {
        unimplemented!()
    }

    pub fn write_result(&mut self) {
        let finished_rs_entries = self.rs.pop_finished();
        let finished_lb_entries = self.lb.pop_finished();
        for entry in finished_rs_entries {
            self.process_rs_entry(entry);
        }
        for entry in finished_lb_entries {
            self.process_lb_entry(entry);
        }
    }

    pub fn execute(&mut self) {
        // jalr의 실행이 완료되면 pc를 변경
        self.rs.execute();
        self.lb.execute(&self.rob);
    }

    pub fn issue(&mut self) {
        // if a last instruction in ROB is either Jalr or Ecall,
        // stop fetching until the instruction is ready to commit.
        let has_to_stall = self
            .rob
            .iter()
            .rev()
            .next()
            .and_then(|entry| match entry.inst.function {
                Function::Jalr if !entry.is_ready => Some(()),
                Function::Ecall => Some(()),
                _ => None,
            })
            .is_some();
        if has_to_stall {
            return;
        }

        let mut pc = self.reg.pc.read();
        let insts: Vec<_> = (0..2)
            .map(|_| {
                let raw_inst = self.memory.read_inst(pc);
                let retval = (pc, Instruction::new(raw_inst));
                pc += consts::WORD_SIZE as u32;
                retval
            })
            .collect();
        self.reg.pc.write(pc);

        for (pc, inst) in insts {
            let rob_index = self.rob.issue(pc, inst.clone());
            if inst.opcode == crate::instruction::Opcode::Load {
                self.lb.issue(rob_index, &self.rob, &self.reg);
            } else {
                self.rs.issue(rob_index, &self.rob, &self.reg);
            }

            // if jal instruction has issued, stop fetching and change pc
            if let Opcode::Jal = inst.opcode {
                self.reg.pc.write(pc + inst.fields.imm.unwrap());
                break;
            }
        }
    }
    // return true when process ends.
    pub fn run_clock(&mut self) -> Vec<ReorderBufferEntry> {
        let retired_insts = self.commit();
        if self.is_program_finished(&retired_insts) {
            return retired_insts;
        }

        self.write_result();
        self.execute();
        self.issue();
        retired_insts
    }

    // fn fetch(&mut self) {
    //     use consts;
    //     self.if_id.pc = self.reg.pc.read();
    //     self.reg.pc.write(self.if_id.pc + consts::WORD_SIZE as u32);
    //     self.if_id.raw_inst = self.memory.read_inst(self.if_id.pc);
    // }

    // fn decode(&mut self) -> bool {
    //     use instruction::Opcode::*;

    //     if self.is_syscall_after_decode() {
    //         return true;
    //     }

    //     self.id_ex.inst = Instruction::new(self.if_id.raw_inst);
    //     self.id_ex.pc = self.if_id.pc;

    //     if let Some(rs1) = self.id_ex.inst.fields.rs1 {
    //         if let Some(rs1_val) = self.get_register_with_forwarding(rs1, false) {
    //             self.id_ex.A = rs1_val as i32;
    //         } else {
    //             self.id_ex = Default::default();
    //             return true;
    //         }
    //     }

    //     if let Some(rs2) = self.id_ex.inst.fields.rs2 {
    //         if let Some(rs2_val) = self.get_register_with_forwarding(rs2, false) {
    //             self.id_ex.B = rs2_val as i32
    //         } else {
    //             self.id_ex = Default::default();
    //             return true;
    //         }
    //     }

    //     self.id_ex.imm = self.id_ex.inst.fields.imm.unwrap_or(0) as i32;

    //     self.id_ex.target_addr = match self.id_ex.inst.opcode {
    //         Branch | Jal => self.id_ex.imm + self.id_ex.pc as i32,
    //         Jalr => (self.id_ex.imm + self.id_ex.A as i32) & (!0x1i32),
    //         _ => 0,
    //     } as u32;

    //     self.if_id = Default::default();
    //     false
    // }

    // fn execute(&mut self) -> bool {
    //     use alu;

    //     if let LoadFp | StoreFp | Fmadd | Fmsub | Fnmadd | Fnmsub | OpFp = self.id_ex.inst.opcode {

    //     } else if Function::Ecall == self.id_ex.inst.function {
    //         if !self.empty_after_execute() {
    //             return true;
    //         }
    //         self.ex_mem.pc = self.id_ex.pc;
    //         self.ex_mem.inst = self.id_ex.inst.clone();
    //         if let 93 | 94 = self.reg.gpr[consts::SYSCALL_NUM_REG].read() {
    //             // It's exit system call!
    //             self.is_finished = true;
    //             return true;
    //         } else {
    //             // do syscall
    //             let ret_val: u32 = match self.reg.gpr[consts::SYSCALL_NUM_REG].read() {
    //                 64 => {
    //                     let fd = self.reg.gpr[consts::SYSCALL_ARG1_REG].read() as i32;
    //                     let buf_addr = self.reg.gpr[consts::SYSCALL_ARG2_REG].read();
    //                     let count = self.reg.gpr[consts::SYSCALL_ARG3_REG].read();
    //                     let bytes = self.memory.read_bytes(buf_addr, count as usize);

    //                     nix::unistd::write(fd, bytes).unwrap() as u32
    //                 }
    //                 78 => {
    //                     let buf_addr = self.reg.gpr[consts::SYSCALL_ARG3_REG].read();
    //                     let buf_size = self.reg.gpr[consts::SYSCALL_ARG4_REG].read();
    //                     let path_addr = self.reg.gpr[consts::SYSCALL_ARG2_REG].read();
    //                     let fd = self.reg.gpr[consts::SYSCALL_ARG1_REG].read();

    //                     let path_addr = self.memory.read_bytes(path_addr, 1).as_ptr() as *const i8;
    //                     let path_str = unsafe { std::ffi::CStr::from_ptr(path_addr) }
    //                         .to_str()
    //                         .expect("Can't convert bytes to str");
    //                     let buf = self.memory.read_bytes_mut(buf_addr, buf_size as usize);
    //                     let contents = nix::fcntl::readlinkat(fd as i32, path_str, buf)
    //                         .expect("Can't call readlinkat system call");
    //                     contents.len() as u32
    //                 }
    //                 80 => {
    //                     let fd = self.reg.gpr[consts::SYSCALL_ARG1_REG].read();
    //                     let buf_addr = self.reg.gpr[consts::SYSCALL_ARG2_REG].read();
    //                     let stat = nix::sys::stat::fstat(fd as i32).unwrap();

    //                     self.memory.write(buf_addr, stat).unwrap_or_else(|err| {
    //                         panic!("{}, in {:?}, registers: {}", err, self.ex_mem, self.reg)
    //                     });
    //                     0
    //                 }
    //                 160 => {
    //                     let addr = self.reg.gpr[consts::SYSCALL_ARG1_REG].read();
    //                     self.memory
    //                         .write(addr, nix::sys::utsname::uname())
    //                         .unwrap_or_else(|err| {
    //                             panic!("{}, in {:?}, registers: {}", err, self.ex_mem, self.reg)
    //                         });
    //                     0
    //                 }
    //                 174 => nix::unistd::getuid().as_raw(),
    //                 175 => nix::unistd::geteuid().as_raw(),
    //                 176 => nix::unistd::getgid().as_raw(),
    //                 177 => nix::unistd::getegid().as_raw(),
    //                 214 => {
    //                     let addr = self.reg.gpr[consts::SYSCALL_ARG1_REG].read();
    //                     let max_mem_addr = self.memory.v_address_range.1;
    //                     if max_mem_addr <= addr {
    //                         self.memory
    //                             .data
    //                             .resize((addr - max_mem_addr + 1) as usize, 0);
    //                         self.memory.v_address_range.1 = addr + 1;
    //                     }
    //                     addr
    //                 }
    //                 a => panic!(
    //                     "(in {:x})system call #{} is not implemented yet",
    //                     self.id_ex.pc, a
    //                 ),
    //             };
    //             self.reg.gpr[consts::SYSCALL_RET_REG].write(ret_val);
    //         }
    //     } else {
    //         self.ex_mem.pc = self.id_ex.pc;
    //         self.ex_mem.inst = self.id_ex.inst.clone();
    //         let (input1, input2) = match self.id_ex.inst.opcode {
    //             OpImm | Lui | Load | LoadFp | Store | StoreFp => {
    //                 (self.id_ex.A, self.id_ex.imm as i32)
    //             }
    //             AuiPc => (self.id_ex.pc as i32, self.id_ex.imm),
    //             Jal | Jalr => (self.id_ex.pc as i32, 4),
    //             _ => (self.id_ex.A, self.id_ex.B),
    //         };
    //         self.ex_mem.alu_result = alu::alu(&self.id_ex.inst.function, input1, input2);
    //         self.ex_mem.B = self.id_ex.B;
    //         self.ex_mem.A = self.id_ex.A;
    //         self.ex_mem.target_addr = self.id_ex.target_addr;

    //         if let Load | Store | Amo = self.ex_mem.inst.opcode {
    //             self.ex_mem.remaining_clock = 10;
    //             if self.ex_mem.inst.opcode == Amo
    //                 && self.ex_mem.inst.function != Function::Lrw
    //                 && self.ex_mem.inst.function != Function::Scw
    //             {
    //                 self.ex_mem.remaining_clock += 10;
    //             }
    //         }
    //     }

    //     self.id_ex = Default::default();
    //     false
    // }

    // fn memory_access(&mut self) -> bool {
    //     // Atomic 명령어 처리를 이 단계에서 해야함.
    //     self.mem_wb.pc = self.ex_mem.pc;
    //     self.mem_wb.inst = self.ex_mem.inst.clone();

    //     let is_stall = match self.ex_mem.inst.opcode {
    //         Branch if self.ex_mem.alu_result == 1 => {
    //             self.reg.pc.write(self.ex_mem.target_addr);
    //             self.id_ex = Default::default();
    //             self.if_id = Default::default();
    //             false
    //         }
    //         Jal | Jalr => {
    //             self.reg.pc.write(self.ex_mem.target_addr);
    //             self.mem_wb.alu_result = self.ex_mem.alu_result;
    //             self.id_ex = Default::default();
    //             self.if_id = Default::default();
    //             false
    //         }
    //         Load => {
    //             if self.ex_mem.remaining_clock > 1 {
    //                 self.ex_mem.remaining_clock -= 1;
    //                 self.mem_wb = Default::default();
    //                 return true;
    //             }
    //             let addr = self.ex_mem.alu_result as u32;
    //             self.mem_wb.mem_result = match self.ex_mem.inst.function {
    //                 Function::Lb => self.memory.read::<i8>(addr) as u32,
    //                 Function::Lbu => self.memory.read::<u8>(addr) as u32,
    //                 Function::Lh => self.memory.read::<i16>(addr) as u32,
    //                 Function::Lhu => self.memory.read::<u16>(addr) as u32,
    //                 Function::Lw => self.memory.read::<u32>(addr) as u32,
    //                 _ => unreachable!(),
    //             };
    //             false
    //         }
    //         Store => {
    //             if self.ex_mem.remaining_clock > 1 {
    //                 self.ex_mem.remaining_clock -= 1;
    //                 self.mem_wb = Default::default();
    //                 return true;
    //             }
    //             let addr = self.ex_mem.alu_result as u32;
    //             let result = match self.ex_mem.inst.function {
    //                 Function::Sb => self.memory.write(addr, self.ex_mem.B as u8),
    //                 Function::Sh => self.memory.write(addr, self.ex_mem.B as u16),
    //                 Function::Sw => self.memory.write(addr, self.ex_mem.B as u32),
    //                 _ => unreachable!(),
    //             };
    //             result.unwrap_or_else(|err| {
    //                 panic!("{}, in {:?}, registers: {}", err, self.ex_mem, self.reg)
    //             });
    //             false
    //         }
    //         Amo => {
    //             if self.ex_mem.A & 0b11 != 0 {
    //                 panic!(
    //                     "Memory misaligned exception on pc:{:x}, inst: {:?}",
    //                     self.ex_mem.pc, self.ex_mem.inst
    //                 );
    //             }
    //             if self.ex_mem.remaining_clock > 1 {
    //                 self.ex_mem.remaining_clock -= 1;
    //                 self.mem_wb = Default::default();
    //                 return true;
    //             }
    //             match self.ex_mem.inst.function {
    //                 Function::Lrw => {
    //                     self.mem_wb.mem_result = self.memory.read::<u32>(self.ex_mem.A as u32);
    //                 }
    //                 Function::Scw => {
    //                     self.memory
    //                         .write(self.ex_mem.A as u32, self.ex_mem.B)
    //                         .unwrap_or_else(|err| {
    //                             panic!("{}, in {:?}, registers: {}", err, self.ex_mem, self.reg)
    //                         });
    //                 }
    //                 _ => {
    //                     self.mem_wb.mem_result = self.memory.read::<u32>(self.ex_mem.A as u32);
    //                     let (a_val, b_val) = (self.mem_wb.mem_result, self.ex_mem.B as u32);
    //                     let new_val = match self.ex_mem.inst.function {
    //                         Function::Amoswapw => b_val,
    //                         Function::Amoaddw => a_val + b_val,
    //                         Function::Amoandw => a_val & b_val,
    //                         Function::Amoorw => a_val | b_val,
    //                         Function::Amoxorw => a_val ^ b_val,
    //                         Function::Amomaxw => std::cmp::max(a_val as i32, b_val as i32) as u32,
    //                         Function::Amomaxuw => std::cmp::max(a_val, b_val),
    //                         Function::Amominw => std::cmp::min(a_val as i32, b_val as i32) as u32,
    //                         Function::Amominuw => std::cmp::min(a_val, b_val),
    //                         _ => unreachable!(),
    //                     };
    //                     self.memory
    //                         .write(self.ex_mem.A as u32, new_val)
    //                         .unwrap_or_else(|err| {
    //                             panic!("{}, in {:?}, registers: {}", err, self.ex_mem, self.reg)
    //                         });
    //                 }
    //             }
    //             false
    //         }
    //         _ => {
    //             self.mem_wb.alu_result = self.ex_mem.alu_result;
    //             false
    //         }
    //     };

    //     if !is_stall {
    //         self.ex_mem = Default::default();
    //     }

    //     is_stall
    // }

    // fn write_back(&mut self) -> MemWbRegister {
    //     use consts;
    //     use instruction::Opcode;
    //     {
    //         let mem_wb = &self.mem_wb;
    //         let inst = &mem_wb.inst;
    //         if let Some(rd) = inst.fields.rd {
    //             let rd = rd as usize;
    //             match inst.opcode {
    //                 LoadFp | StoreFp | Fmadd | Fmsub | Fnmadd | Fnmsub | OpFp => unreachable!(),
    //                 Opcode::Store | Opcode::Branch => {}
    //                 Opcode::Load | Amo => self.reg.gpr[rd].write(mem_wb.mem_result),
    //                 _ => {
    //                     self.reg.gpr[rd].write(mem_wb.alu_result as u32);
    //                 }
    //             }

    //             if mem_wb.fp_add_inst.value != consts::NOP {
    //                 if let Some(rd) = mem_wb.fp_add_inst.fields.rd {
    //                     self.reg.fpr[rd as usize].write(mem_wb.fp_add_result);
    //                 }
    //             }
    //             if mem_wb.fp_mul_inst.value != consts::NOP {
    //                 if let Some(rd) = mem_wb.fp_mul_inst.fields.rd {
    //                     self.reg.fpr[rd as usize].write(mem_wb.fp_mul_result);
    //                 }
    //             }
    //             if mem_wb.fp_div_inst.value != consts::NOP {
    //                 if let Some(rd) = mem_wb.fp_div_inst.fields.rd {
    //                     self.reg.fpr[rd as usize].write(mem_wb.fp_div_result);
    //                 }
    //             }
    //         }
    //     }

    //     let mut new_reg = MemWbRegister::default();
    //     std::mem::swap(&mut self.mem_wb, &mut new_reg);
    //     new_reg
    // }

    // fn get_register_with_forwarding(&self, reg_num: u8, is_fp_register: bool) -> Option<u32> {
    //     if is_fp_register {
    //         unimplemented!()
    //     } else {
    //         let mut ex_val = None;
    //         if Some(reg_num) == self.ex_mem.inst.fields.rd {
    //             if let Load | Amo = self.ex_mem.inst.opcode {
    //                 return None;
    //             }
    //             ex_val = match self.ex_mem.inst.opcode {
    //                 Store | Branch => None,
    //                 _ => Some(self.ex_mem.alu_result as u32),
    //             }
    //         }

    //         let inst = &self.mem_wb.inst;
    //         let mem_opcode = inst.opcode;
    //         ex_val.or_else(|| {
    //             if mem_opcode != Store && mem_opcode != Branch && Some(reg_num) == inst.fields.rd {
    //                 match mem_opcode {
    //                     Load | Amo => Some(self.mem_wb.mem_result),
    //                     _ => Some(self.mem_wb.alu_result as u32),
    //                 }
    //             } else {
    //                 Some(self.reg.gpr[reg_num as usize].read())
    //             }
    //         })
    //     }
    // }

    // fn empty_after_execute(&self) -> bool {
    //     self.ex_mem.inst.is_nop() && self.mem_wb.inst.is_nop()
    // }
    // fn is_syscall_after_decode(&self) -> bool {
    //     [
    //         self.id_ex.inst.function,
    //         self.ex_mem.inst.function,
    //         self.mem_wb.inst.function,
    //     ]
    //     .iter()
    //     .any(|f| *f == Function::Ecall)
    // }
}