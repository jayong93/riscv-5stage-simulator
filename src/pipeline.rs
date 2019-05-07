//! Pipeline definition.

use instruction::{Instruction, Function};
use instruction::Opcode::*;
use memory;
use register;
use std::collections::VecDeque;

/// Pipeline holding four inter-stage registers
#[derive(Debug)]
pub struct Pipeline {
    pub reg: register::RegisterFile,
    pub memory: memory::ProcessMemory,
    pub if_id: IfIdRegister,
    pub id_ex: IdExRegister,
    pub ex_mem: ExMemRegister,
    pub fp_add_pending: VecDeque<FpuPendingInstruction>,
    pub fp_mul_pending: VecDeque<FpuPendingInstruction>,
    pub fp_div_pending: VecDeque<FpuPendingInstruction>,
    pub fp_add_mem: FpMemRegister,
    pub fp_mul_mem: FpMemRegister,
    pub fp_div_mem: FpMemRegister,
    pub mem_wb: MemWbRegister,
}

impl Pipeline {
    pub fn new(entry_point: u32, memory: memory::ProcessMemory) -> Pipeline {
        Pipeline {
            reg: register::RegisterFile::new(entry_point),
            memory,
            if_id: Default::default(),
            id_ex: Default::default(),
            ex_mem: Default::default(),
            fp_add_pending: Default::default(),
            fp_mul_pending: Default::default(),
            fp_div_pending: Default::default(),
            fp_add_mem: Default::default(),
            fp_mul_mem: Default::default(),
            fp_div_mem: Default::default(),
            mem_wb: Default::default(),
        }
    }

    // return true when process ends.
    pub fn run_clock(&mut self) -> bool {
        self.write_back();
        self.memory_access();
        let result = self.execute();
        self.decode();
        self.fetch();
        result
    }

    fn fetch(&mut self) {
        use consts;
        self.if_id.pc = self.reg.pc.read();
        self.reg.pc.write(self.if_id.pc + consts::WORD_SIZE as u32);
        self.if_id.raw_inst = self.memory.read_inst(self.if_id.pc);
    }

    fn decode(&mut self) {
        use instruction::Opcode::*;

        self.id_ex.inst = Instruction::new(self.if_id.raw_inst);
        self.id_ex.pc = self.if_id.pc;
        let rs1 = self.id_ex.inst.fields.rs1;
        if let Some(rs1_val) = self.get_register_with_forwarding(rs1, false) {
            self.id_ex.rs1 = rs1_val as i32;
        } else {
            self.id_ex = Default::default();
            return;
        }

        if let OpImm | Lui | AuiPc | Load | LoadFp = self.id_ex.inst.opcode {
            self.id_ex.rs2 = self.id_ex.inst.fields.imm as i32;
        }
        else {
            let rs2 = self.id_ex.inst.fields.rs2;
            if let Some(rs2_val) = self.get_register_with_forwarding(rs2, false) {
                self.id_ex.rs2 = rs2_val as i32
            } else {
                self.id_ex = Default::default();
                return
            }
        }

        self.id_ex.target_addr = match self.id_ex.inst.opcode {
            Branch | Jal => self.id_ex.inst.fields.imm + self.id_ex.pc,
            Jalr => {
                self.id_ex.inst.fields.imm + self.id_ex.rs1 as u32
            }
            _ => 0,
        };
    }

    fn execute(&mut self) -> bool {
        use alu;
        if let LoadFp | StoreFp | Fmadd | Fmsub | Fnmadd | Fnmsub | OpFp =
            self.id_ex.inst.opcode
        {

        } else if Function::Ecall == self.id_ex.inst.function
            && self.reg.gpr[17].read() == 60
        {
            // It's exit system call!
            return true;
        } else {
            self.ex_mem.pc = self.id_ex.pc;
            self.ex_mem.inst = self.id_ex.inst.clone();
            self.ex_mem.alu_result = alu::alu(&self.id_ex);
            self.ex_mem.rs2 = self.id_ex.rs2;
            self.ex_mem.target_addr = self.id_ex.target_addr;
        }
        return false;
    }

    fn memory_access(&mut self) {
        // Atomic 명령어 처리를 이 단계에서 해야함.
        self.mem_wb.pc = self.ex_mem.pc;
        self.mem_wb.inst = self.ex_mem.inst.clone();

        match self.ex_mem.inst.opcode {
            Branch if self.ex_mem.alu_result == 1 => {
                self.reg.pc.write(self.ex_mem.target_addr);
                self.ex_mem = Default::default();
                self.id_ex = Default::default();
                self.if_id = Default::default();
            },
            Jal | Jalr => {
                self.reg.pc.write(self.ex_mem.target_addr);
                self.ex_mem = Default::default();
                self.id_ex = Default::default();
                self.if_id = Default::default();
            },
            Load => {
                let addr = self.ex_mem.alu_result as u32;
                self.mem_wb.mem_result = match self.ex_mem.inst.function {
                    Function::Lb => self.memory.read::<i8>(addr) as u32,
                    Function::Lbu => self.memory.read::<u8>(addr) as u32,
                    Function::Lh => self.memory.read::<i16>(addr) as u32,
                    Function::Lhu => self.memory.read::<u16>(addr) as u32,
                    Function::Lw => self.memory.read::<u32>(addr) as u32,
                    _ => unreachable!()
                }
            }
            Store => {
                let addr = self.ex_mem.alu_result as u32;
                self.memory.write(addr, self.ex_mem.rs2 as u32);
            }
            _ => {
                self.mem_wb.alu_result = self.ex_mem.alu_result;
            },
        }
    }

    fn write_back(&mut self) {
        use consts;
        use instruction::Opcode;

        let mem_wb = &self.mem_wb;
        let inst = &mem_wb.inst;
        let rd = inst.fields.rd as usize;
        let npc = mem_wb.pc + consts::WORD_SIZE as u32;
        match inst.opcode {
            LoadFp | StoreFp | Fmadd | Fmsub | Fnmadd | Fnmsub | OpFp => unreachable!(),
            Opcode::Store | Opcode::Branch => {}
            Opcode::Load => {
                self.reg.gpr[rd].write(self.mem_wb.mem_result);
            }
            Opcode::Lui => {
                self.reg.gpr[rd].write(inst.fields.imm);
            }
            Opcode::Jal | Opcode::Jalr => {
                self.reg.gpr[rd].write(npc);
            }
            _ => {
                self.reg.gpr[rd].write(mem_wb.alu_result as u32);
            }
        }

        if mem_wb.fp_add_inst.value != consts::NOP {
            let rd = mem_wb.fp_add_inst.fields.rd as usize;
            self.reg.fpr[rd].write(mem_wb.fp_add_result);
        }
        if mem_wb.fp_mul_inst.value != consts::NOP {
            let rd = mem_wb.fp_mul_inst.fields.rd as usize;
            self.reg.fpr[rd].write(mem_wb.fp_mul_result);
        }
        if mem_wb.fp_div_inst.value != consts::NOP {
            let rd = mem_wb.fp_div_inst.fields.rd as usize;
            self.reg.fpr[rd].write(mem_wb.fp_div_result);
        }
    }

    fn get_register_with_forwarding(&self, reg_num: u8, is_fp_register: bool) -> Option<u32> {
        if is_fp_register {
            unimplemented!()
        } else {
            let mut ex_val = None;
            if reg_num == self.ex_mem.inst.fields.rd {
                if self.ex_mem.inst.opcode == Load {
                    return None
                }
                ex_val = match self.ex_mem.inst.opcode {
                    Store => None,
                    _ => Some(self.ex_mem.alu_result as u32)
                }
            } 

            ex_val.or_else(|| {
                if reg_num == self.mem_wb.inst.fields.rd && self.mem_wb.inst.opcode == Load {
                    Some(self.mem_wb.mem_result)
                } else {
                    Some(self.reg.gpr[reg_num as usize].read())
                }
            })
        }
    }
}

/// Pipeline register between instruction fetch and instruction decode stages.
#[derive(Clone, Debug)]
pub struct IfIdRegister {
    /// Program Counter
    pub pc: u32,

    /// Raw instruction
    pub raw_inst: u32,
}

impl Default for IfIdRegister {
    fn default() -> Self {
        use consts;
        IfIdRegister {
            pc: 0,
            raw_inst: consts::NOP, // NOP
        }
    }
}
impl IfIdRegister {
    pub fn new() -> Self {
        Default::default()
    }
}

/// Pipeline register between instruction decode and execution stages.
#[derive(Clone, Default, Debug)]
pub struct IdExRegister {
    pub pc: u32,
    pub inst: Instruction,
    pub rs1: i32,
    pub rs2: i32,
    pub target_addr: u32,
}

impl IdExRegister {
    pub fn new() -> Self {
        Default::default()
    }
}

/// Pipeline register between execution and memory stages.
#[derive(Clone, Default, Debug)]
pub struct ExMemRegister {
    pub pc: u32,
    pub inst: Instruction,
    pub alu_result: i32,
    pub rs2: i32,
    pub target_addr: u32,
}

impl ExMemRegister {
    pub fn new() -> ExMemRegister {
        Default::default()
    }
}

#[derive(Clone, Debug)]
pub struct FpuPendingInstruction {
    pub pc: u32,
    pub inst: Instruction,
    pub remaining_clock: u8,
}

#[derive(Clone, Default, Debug)]
pub struct FpMemRegister {
    pub pc: u32,
    pub inst: Instruction,
    pub fpu_result: f32,
    pub rs2: i32,
}

impl FpMemRegister {
    pub fn new() -> Self {
        Default::default()
    }
}

/// Pipeline register between memory and writeback stages.
#[derive(Clone, Default, Debug)]
pub struct MemWbRegister {
    pub pc: u32,
    pub inst: Instruction,
    pub alu_result: i32,
    pub fp_add_inst: Instruction,
    pub fp_add_result: f32,
    pub fp_mul_inst: Instruction,
    pub fp_mul_result: f32,
    pub fp_div_inst: Instruction,
    pub fp_div_result: f32,
    pub mem_result: u32,
}

impl MemWbRegister {
    pub fn new() -> MemWbRegister {
        Default::default()
    }
}
