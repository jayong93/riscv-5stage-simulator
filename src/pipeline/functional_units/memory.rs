use instruction::{Opcode, Function};
use memory::ProcessMemory;
use pipeline::exception::Exception;
use pipeline::operand::Operand;
use pipeline::reorder_buffer::ReorderBufferEntry;

pub struct MemoryUnit();

impl MemoryUnit {
    pub fn execute_store(store_entry: &mut ReorderBufferEntry, mem: &mut ProcessMemory) {
        use self::Function::*;
        if let Opcode::Amo = store_entry.inst.opcode {
            if store_entry.reg_value.is_none() {
                return;
            }
        }

        if let (Operand::Value(addr), Operand::Value(value)) =
            (store_entry.addr, store_entry.mem_value)
        {
            match store_entry.inst.function {
                Sb => mem.write(addr, value as u8),
                Sh => mem.write(addr, value as u16),
                _ => mem.write(addr, value as u32),
            }
            .unwrap();
            store_entry.mem_rem_cycle = 0;
        }
    }

    pub fn execute(
        addr: u32,
        load_entry: &mut ReorderBufferEntry,
        mem: &ProcessMemory,
    ) -> Result<u32, Exception> {
        use self::Function::*;
        // Store 확인은 Load Buffer에서 할 일 이므로 여기선 처리 안해도 됨.
        let value = match load_entry.inst.function {
            Lb => mem.read::<i8>(addr).map(|val| val as u32),
            Lbu => mem.read::<u8>(addr).map(|val| val as u32),
            Lh => mem.read::<i16>(addr).map(|val| val as u32),
            Lhu => mem.read::<u16>(addr).map(|val| val as u32),
            Lw => mem.read::<u32>(addr),
            _ => {
                let value = mem.read::<u32>(addr)?;
                let value_to_calc = if let Operand::Value(val) = load_entry.mem_value {
                    val
                } else {
                    unreachable!()
                };

                if let Lrw = load_entry.inst.function {
                } else {
                    load_entry.mem_rem_cycle = crate::consts::MEM_CYCLE;
                }

                let mem_val = match load_entry.inst.function {
                    Lrw => 0,
                    Amoaddw => value + value_to_calc,
                    Amoandw => value & value_to_calc,
                    Amoorw => value | value_to_calc,
                    Amoxorw => value ^ value_to_calc,
                    Amomaxuw => std::cmp::max(value, value_to_calc),
                    Amomaxw => std::cmp::max(value as i32, value_to_calc as i32) as u32,
                    Amominuw => std::cmp::min(value, value_to_calc),
                    Amominw => std::cmp::min(value as i32, value_to_calc as i32) as u32,
                    Amoswapw => value_to_calc,
                    _ => unreachable!(),
                };
                load_entry.mem_value = Operand::Value(mem_val);
                Ok(value)
            }
        };
        value
    }
}
