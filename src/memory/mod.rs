//! Harvard architecture (separate instruction and data) memory interface.

use byteorder::{LittleEndian, ReadBytesExt};
use goblin::elf32::program_header::ProgramHeader as Elf32ProgramHeader;
use std::mem::size_of;

mod consts;

#[repr(C)]
struct AuxVec {
    vec_type: u32,
    data: u32,
}

impl AuxVec {
    fn new(vec_type: u32, data: u32) -> Self {
        Self { vec_type, data }
    }
}

#[derive(Debug, Default)]
pub struct ProcessMemory {
    pub v_address_range: (u32, u32),
    pub read_only_range: (u32, u32),
    pub stack_range: (u32, u32),
    pub data: Vec<u8>,
    pub stack: Vec<u8>,
    pub stack_pointer_init: u32,
}

impl ProcessMemory {
    pub fn new(elf_struct: &goblin::elf::Elf, elf_data: &[u8], program_name: &str) -> Self {
        let mut memory = elf_struct
            .program_headers
            .iter()
            .filter(|header| header.p_type == goblin::elf::program_header::PT_LOAD)
            .fold(ProcessMemory::default(), |mut memory, header| {
                let vm_range = header.vm_range();
                if !header.is_write() {
                    if memory.read_only_range.0 == memory.read_only_range.1 {
                        memory.read_only_range.0 = vm_range.start as u32;
                        memory.read_only_range.1 = vm_range.end as u32;
                    } else {
                        memory.read_only_range.1 = vm_range.end as u32;
                    }
                }

                if memory.v_address_range.0 == memory.v_address_range.1 {
                    memory.data.resize(vm_range.start, 0);
                    memory.v_address_range.1 = vm_range.end as u32;
                } else {
                    let old_size = memory.data.len();
                    if memory.v_address_range.1 < vm_range.start as u32 {
                        memory.data.resize(
                            old_size + (vm_range.start as u32 - memory.v_address_range.1) as usize,
                            0,
                        );
                    }
                    memory.v_address_range.1 = vm_range.end as u32;
                }
                let old_size = memory.data.len();
                memory.data.resize(old_size + (header.p_memsz as usize), 0);
                memory.data[old_size..(old_size + header.p_filesz as usize)]
                    .copy_from_slice(&elf_data[header.file_range()]);
                memory
            });
        memory.initialize_stack(
            8 * 1024 * 1024,
            elf_struct
                .program_headers
                .iter()
                .map(|ph| ph.clone().into())
                .collect::<Vec<_>>()
                .as_slice(),
            program_name,
            elf_struct.entry as u32,
        );
        memory
    }

    // it returns initial value of stack pointer
    fn initialize_stack(
        &mut self,
        stack_size: u32,
        program_headers: &[Elf32ProgramHeader],
        program_name: &str,
        entry_point: u32,
    ) {
        use self::consts::*;

        self.stack.resize(stack_size as usize, 0);
        self.stack_range = (0u32.wrapping_sub(stack_size), 0);

        let sp = 0u32;
        let (sp, header_num) = self.push_program_headers(program_headers, sp);
        let header_addr = sp;
        let sp = self.push_program_name(program_name, sp);
        let program_name_addr = sp;

        let aux_vecs = [
            AuxVec::new(AT_ENTRY, entry_point),
            AuxVec::new(AT_PHNUM, header_num),
            AuxVec::new(AT_PHENT, size_of::<Elf32ProgramHeader>() as u32),
            AuxVec::new(AT_PHDR, header_addr),
            AuxVec::new(AT_PAGESZ, 0),
            AuxVec::new(AT_SECURE, 0),
            AuxVec::new(AT_RANDOM, program_name_addr),
            AuxVec::new(AT_NULL, 0),
        ];
        let arg_values = [1u32, program_name_addr, 0u32, 0u32];

        // align stack pointer
        let aux_vecs_bytes_num = (size_of::<AuxVec>() * aux_vecs.len()) as u32;
        let arg_values_bytes_num = (size_of::<u32>() * arg_values.len()) as u32;
        let total_bytes = aux_vecs_bytes_num + arg_values_bytes_num;
        let next_sp = sp - total_bytes;
        let sp = sp - (next_sp - (next_sp & (-16i32 as u32)));

        let sp = {
            let sp = sp.wrapping_sub(aux_vecs_bytes_num);
            self.write_slice(sp, aux_vecs.as_ref()).unwrap();
            sp
        };
        let sp = {
            let sp = sp.wrapping_sub(arg_values_bytes_num);
            self.write_slice(sp, arg_values.as_ref()).unwrap();
            sp
        };

        self.stack_pointer_init = sp;
    }

    fn push_program_headers(&mut self, headers: &[Elf32ProgramHeader], sp: u32) -> (u32, u32) {
        headers.iter().fold((sp, 0), |(sp, num), header| {
            let sp = sp.wrapping_sub(size_of::<Elf32ProgramHeader>() as u32);
            self.write(sp, *header).unwrap();
            (sp, num + 1)
        })
    }

    fn push_program_name(&mut self, name: &str, sp: u32) -> u32 {
        let name_cstring = std::ffi::CString::new(name.to_owned()).unwrap();
        let name_bytes = name_cstring.as_bytes_with_nul();
        let sp = sp.wrapping_sub(name_bytes.len() as u32);
        self.write_slice(sp, name_bytes).unwrap();
        sp
    }

    fn check_address_space(&self, addr: u32) -> Result<(), String> {
        if addr < self.v_address_range.0
            || (addr >= self.v_address_range.1 && addr < self.stack_range.0)
        {
            Err(format!("{:x} is out of address range.", addr))
        } else {
            Ok(())
        }
    }

    fn check_write_address_space(&self, addr: u32) -> Result<(), String> {
        if self.read_only_range.0 <= addr && addr < self.read_only_range.1 {
            Err(format!("{:x} is out of writable address range.", addr))
        } else {
            Ok(())
        }
    }

    pub fn read_inst(&self, addr: u32) -> u32 {
        self.check_address_space(addr).unwrap();

        let offset = (addr - self.v_address_range.0) as usize;
        let mut data = &(self.data[offset..offset + 4]);
        data.read_u32::<LittleEndian>()
            .expect("Can't read memory as u32 instruction")
    }

    pub fn read<T: Copy>(&self, addr: u32) -> T {
        let data_size = size_of::<T>() as usize;
        let data_ptr = self.read_bytes(addr, data_size).as_ptr() as *const T;
        unsafe { *data_ptr }
    }

    pub fn read_bytes(&self, addr: u32, size: usize) -> &[u8] {
        self.check_address_space(addr).unwrap();

        let buf;
        let offset = if addr < self.stack_range.0 {
            buf = &self.data;
            (addr - self.v_address_range.0) as usize
        } else {
            buf = &self.stack;
            (addr - self.stack_range.0) as usize
        };
        &buf[offset..offset + size]
    }

    pub fn read_bytes_mut(&mut self, addr: u32, size: usize) -> &mut [u8] {
        self.check_address_space(addr).unwrap();

        let buf;
        let offset = if addr < self.stack_range.0 {
            buf = &mut self.data;
            (addr - self.v_address_range.0) as usize
        } else {
            buf = &mut self.stack;
            (addr - self.stack_range.0) as usize
        };
        &mut buf[offset..offset + size]
    }

    pub fn write<T>(&mut self, addr: u32, value: T) -> Result<(), String> {
        let data_size = size_of::<T>() as usize;
        let ptr = &value as *const T as *const u8;
        let byte_slice = unsafe { std::slice::from_raw_parts(ptr, data_size) };
        self.write_slice(addr, byte_slice)
    }

    pub fn write_slice<T>(&mut self, addr: u32, value: &[T]) -> Result<(), String> {
        self.check_address_space(addr)?;
        self.check_write_address_space(addr)?;

        let data_size = size_of::<T>();

        let data;
        if addr < self.stack_range.0 {
            let offset = (addr - self.v_address_range.0) as usize;
            data = &mut (self.data[offset..offset + data_size * value.len()]);
        } else {
            let offset = (addr - self.stack_range.0) as usize;
            data = &mut (self.stack[offset..offset + data_size * value.len()]);
        }

        let ptr = value.as_ptr() as *const u8;
        let byte_slice = unsafe { std::slice::from_raw_parts(ptr, value.len() * data_size) };

        if unsafe { crate::PRINT_STORES } {
            eprintln!("DEBUG: Store has occured in {:x}.", addr);
            eprintln!("DEBUG: val: {:?}", byte_slice);
        }

        data.copy_from_slice(byte_slice);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_memory() -> ProcessMemory {
        let mut memory = ProcessMemory::default();
        memory.initialize_stack(8 * 1024 * 1024, &[], "test_bin", 0);
        memory
    }

    #[test]
    fn test_reading_memory() {
        let mut memory = init_memory();

        let mem_len = memory.stack.len();
        memory.stack[mem_len - 1] = 10;
        memory.stack[mem_len - 2] = 20;
        assert_eq!(memory.read::<u8>(-1i32 as u32), 10);
        assert_eq!(memory.read_bytes(-2i32 as u32, 2), &[20, 10]);
        memory.stack[mem_len - 1] = 0x10;
        memory.stack[mem_len - 2] = 0x20;
        assert_eq!(memory.read::<u16>(-2i32 as u32), 0x1020);
    }

    #[test]
    fn test_writing_memory() {
        let mut memory = init_memory();
        memory.write(-4i32 as u32, 600u32).unwrap();
        assert_eq!(memory.read::<u32>(-4i32 as u32), 600);
        memory.write(-8i32 as u32, 0x12345678u32).unwrap();
        assert_eq!(
            memory.read_bytes(-8i32 as u32, 4),
            &[0x78, 0x56, 0x34, 0x12]
        );

        memory.write(-8i32 as u32, 0xABCDu16).unwrap();
        assert_eq!(
            memory.read_bytes(-8i32 as u32, 4),
            &[0xCD, 0xAB, 0x34, 0x12]
        );

        let arr = [1u8, 2, 3, 4];
        memory.write_slice(-4i32 as u32, arr.as_ref()).unwrap();
        assert_eq!(memory.read::<u32>(-4i32 as u32), 0x04030201);
    }
}
