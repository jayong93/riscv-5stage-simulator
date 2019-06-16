use instruction::Function;
use memory::ProcessMemory;
use pipeline::load_buffer::LoadBuffer;
use pipeline::operand::Operand;
use pipeline::reorder_buffer::{ReorderBufferEntry, ReorderBuffer};

pub struct MemoryUnit();

impl MemoryUnit {
    fn is_store_ready(store: &ReorderBufferEntry, rob: &ReorderBuffer, load_buf: &LoadBuffer) -> bool {
        unimplemented!()
    }

    pub fn execute_store(
        store_entry: &mut ReorderBufferEntry,
        mem: &mut ProcessMemory,
        rob: &ReorderBuffer,
        load_buf: &LoadBuffer,
    ) {
        use self::Function::*;
        if Self::is_store_ready(store_entry, rob, load_buf) {
            if let (Operand::Value(addr), Operand::Value(value)) = (store_entry.addr, store_entry.mem_value) {
                match store_entry.inst.function {
                    Sb => mem.write(addr, value as u8),
                    Sh => mem.write(addr, value as u16),
                    _ => mem.write(addr, value as u32),
                }.unwrap();
                store_entry.mem_rem_cycle = 0;
            }
        }
    }

    pub fn execute(addr: u32, load_entry: &mut ReorderBufferEntry, mem: &ProcessMemory) -> u32 {
        use self::Function::*;
        // Store 확인은 Load Buffer에서 할 일 이므로 여기선 처리 안해도 됨.
        let value = match load_entry.inst.function {
            Lb => mem.read::<i8>(addr) as u32,
            Lbu => mem.read::<u8>(addr) as u32,
            Lh => mem.read::<i16>(addr) as u32,
            Lhu => mem.read::<u16>(addr) as u32,
            Lw => mem.read::<u32>(addr),
            _ => {
                let value = mem.read::<u32>(addr);
                let value_to_calc = if let Operand::Value(val) = load_entry.mem_value {
                    val
                } else {
                    unreachable!()
                };
                let mem_val = match load_entry.inst.function {
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
                value
            }
        };
        value
    }
}
