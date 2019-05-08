//! Harvard architecture (separate instruction and data) memory interface.

use byteorder::{LittleEndian, ReadBytesExt};
use std::mem::size_of;

#[derive(Debug, Default)]
pub struct ProcessMemory {
    v_address_range: (u32, u32),
    read_only_range: (u32, u32),
    stack_range: (u32, u32),
    data: Vec<u8>,
    stack: Vec<u8>,
}

impl ProcessMemory {
    pub fn new(elf_struct: &goblin::elf::Elf, elf_data: &[u8]) -> Self {
        let mut memory = elf_struct
            .program_headers
            .iter()
            .filter(|header| {
                header.p_type == goblin::elf::program_header::PT_LOAD
            })
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
                    memory.v_address_range.0 = vm_range.start as u32;
                    memory.v_address_range.1 = vm_range.end as u32;
                } else {
                    let old_size = memory.data.len();
                    if memory.v_address_range.1 < vm_range.start as u32 {
                        memory.data.resize(
                            old_size
                                + (vm_range.start as u32
                                    - memory.v_address_range.1)
                                    as usize,
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
        memory.stack.resize(8*1024*1024, 0);
        memory.stack_range = (0u32.overflowing_sub(memory.stack.len() as u32).0, 0);
        memory
    }

    fn check_address_space(&self, addr: u32) {
        if addr < self.v_address_range.0 || (addr >= self.v_address_range.1 && addr < self.stack_range.0) {
            panic!("{:x} is out of address range.", addr);
        }
    }

    fn check_write_address_space(&self, addr: u32) {
        if self.read_only_range.0 <= addr && addr < self.read_only_range.1 {
            panic!("{:x} is out of writable address range.", addr);
        }
    }

    pub fn read_inst(&self, addr: u32) -> u32 {
        self.check_address_space(addr);

        let offset = (addr - self.v_address_range.0) as usize;
        let mut data = &(self.data[offset..offset + 4]);
        data.read_u32::<LittleEndian>()
            .expect("Can't read memory as u32 instruction")
    }

    pub fn read<T: num_traits::Num + Copy>(&self, addr: u32) -> T {
        self.check_address_space(addr);

        let data_size = size_of::<T>() as usize;
        let data;
        if addr < self.stack_range.0 {
            let offset = (addr - self.v_address_range.0) as usize;
            data = &(self.data[offset..offset + data_size]);
        } else {
            let offset = (addr-self.stack_range.0) as usize;
            data = &(self.stack[offset..offset + data_size]);
        }
        let data_ptr = data.as_ptr() as *const T;
        unsafe { *std::slice::from_raw_parts(data_ptr, 1).get_unchecked(0) }
    }

    pub fn write<T: num_traits::Num>(&mut self, addr: u32, value: T) {
        self.check_address_space(addr);
        self.check_write_address_space(addr);

        let data_size = size_of::<T>() as usize;
        let data;
        if addr < self.stack_range.0 {
            let offset = (addr - self.v_address_range.0) as usize;
            data = &mut (self.data[offset..offset + data_size]);
        } else {
            let offset = (addr - self.stack_range.0) as usize;
            data = &mut (self.stack[offset..offset + data_size]);
        }
        let ptr = &value as *const T as *const u8;
        let byte_slice = unsafe { std::slice::from_raw_parts(ptr, data_size) };
        data.copy_from_slice(byte_slice);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn test_reading_elf() {
        let mut elf_data = Vec::new();
        let _ = std::fs::File::open("tests/hello")
            .unwrap()
            .read_to_end(&mut elf_data)
            .unwrap();
        let elf = goblin::elf::Elf::parse(&elf_data).unwrap();
        let image = ProcessMemory::new(&elf, &elf_data);
        let entry_point = elf.entry as u32;
        assert_eq!(image.v_address_range.0, 0x10000);
        assert_eq!(image.v_address_range.1, 0x7fd08 + 0x1e0c);
        assert_eq!(image.read_only_range.0, 0x10000);
        assert_eq!(image.read_only_range.1, 0x10000 + 0x6dd46);
        assert_eq!(entry_point, 0x10338);
        assert_eq!(image.data.len(), 0x7fd08 + 0x1e0c - 0x10000);
    }

    #[test]
    fn test_reading_memory() {
        let mut elf_data = Vec::new();
        let _ = std::fs::File::open("tests/hello")
            .unwrap()
            .read_to_end(&mut elf_data)
            .unwrap();
        let elf = goblin::elf::Elf::parse(&elf_data).unwrap();
        let image = ProcessMemory::new(&elf, &elf_data);
        let entry_point = elf.entry as u32;

        assert_eq!(image.read_inst(entry_point), 0x034000ef);
        let str_bytes = (0x625b0..0x625b0 + 13)
            .map(|addr| image.read::<u8>(addr))
            .collect::<Vec<u8>>();
        assert_eq!(String::from_utf8(str_bytes).unwrap(), "Hello, World!");
    }

    #[test]
    fn test_writing_memory() {
        let mut elf_data = Vec::new();
        let _ = std::fs::File::open("tests/hello")
            .unwrap()
            .read_to_end(&mut elf_data)
            .unwrap();
        let elf = goblin::elf::Elf::parse(&elf_data).unwrap();
        let mut image = ProcessMemory::new(&elf, &elf_data);

        image.write(0x7fd08, b'A');
        let result: char = image.read::<u8>(0x7fd08).into();
        assert_eq!(result, 'A');
        image.write(0x80000, 100u32);
        assert_eq!(image.read::<u32>(0x80000), 100);
        image.write(0x80000, -10i8);
        assert_eq!(image.read::<i8>(0x80000), -10);
        image.write(0x80000, 3.14f32);
        assert_eq!(image.read::<f32>(0x80000), 3.14);
    }

    #[test]
    #[should_panic]
    fn test_writing_to_read_only_memory() {
        let mut elf_data = Vec::new();
        let _ = std::fs::File::open("tests/hello")
            .unwrap()
            .read_to_end(&mut elf_data)
            .unwrap();
        let elf = goblin::elf::Elf::parse(&elf_data).unwrap();
        let mut image = ProcessMemory::new(&elf, &elf_data);

        image.write(0x20000, 100);
    }

    #[test]
    fn test_rw_stack() {
        let mut elf_data = Vec::new();
        let _ = std::fs::File::open("tests/hello")
            .unwrap()
            .read_to_end(&mut elf_data)
            .unwrap();
        let elf = goblin::elf::Elf::parse(&elf_data).unwrap();
        let mut image = ProcessMemory::new(&elf, &elf_data);

        image.write(0u32.overflowing_sub(10).0, 911u32);
        assert_eq!(image.read::<u32>(0u32.overflowing_sub(10).0), 911u32);
    }
}
