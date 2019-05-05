//! Harvard architecture (separate instruction and data) memory interface.

pub mod data;
pub mod instruction;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::cmp::Ordering;
use std::io::Read;
use std::mem::size_of;
use std::ops::Range;

#[derive(Debug, Default)]
pub struct ProcessMemory {
    v_address_range: (u32, u32),
    read_only_range: (u32, u32),
    data: Vec<u8>,
}

impl ProcessMemory {
    pub fn new(elf_struct: &goblin::elf::Elf, elf_data: &[u8]) -> Self {
        elf_struct
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
                        memory.v_address_range.0 = vm_range.start as u32;
                        memory.v_address_range.1 = vm_range.end as u32;
                    } else {
                        let old_size = memory.data.len();
                        if memory.v_address_range.1 < vm_range.start as u32 {
                            memory.data.resize(old_size + (vm_range.start as u32 - memory.v_address_range.1) as usize, 0);
                        }
                        memory.v_address_range.1 = vm_range.end as u32;
                    }
                    let old_size = memory.data.len();
                    memory.data.resize(old_size + (header.p_memsz as usize), 0);
                    memory.data[old_size..(old_size + header.p_filesz as usize)]
                        .copy_from_slice(&elf_data[header.file_range()]);
                    memory
                },
            )
    }

    fn check_address_space(&self, addr: u32) {
        if addr < self.v_address_range.0 || addr >= self.v_address_range.1 {
            panic!("{} is out of address range.", addr);
        }
    }
    
    fn check_write_address_space(&self, addr: u32) {
        if self.read_only_range.0 <= addr && addr < self.read_only_range.1 {
            panic!("{} is out of writable address range.", addr);
        }
    }

    pub fn read_inst(&self, addr: u32) -> u32 {
        self.check_address_space(addr);

        let offset = (addr - self.v_address_range.0) as usize;
        let mut data = &(self.data[offset..offset+4]);
        data.read_u32::<LittleEndian>().expect("Can't read memory as u32 instruction")
    }
    
    pub fn read_int<T: num_traits::PrimInt + num_traits::FromPrimitive>(&self, addr: u32) -> T {
        self.check_address_space(addr);
        
        let offset = (addr - self.v_address_range.0) as usize;
        let data_size = size_of::<T>() as usize;
        let mut data = &(self.data[offset..offset+data_size]);
        let val = data.read_uint::<LittleEndian>(data_size).expect("Can't read a memory as u64");
        T::from_u64(val).unwrap()
    }

    pub fn read_float(&self, addr: u32) -> f32 {
        self.check_address_space(addr);

        let offset = (addr - self.v_address_range.0) as usize;
        let mut data = &(self.data[offset..offset+4]);
        data.read_f32::<LittleEndian>().expect("Can't read a memory as u64")
    }

    pub fn write_int<T: num_traits::PrimInt + num_traits::AsPrimitive<u64>>(&mut self, addr: u32, value: T) {
        self.check_address_space(addr);
        self.check_write_address_space(addr);

        let offset = (addr-self.v_address_range.0) as usize;
        let data_size = size_of::<T>() as usize;
        let mut data = &mut (self.data[offset..offset+data_size]);
        // transmute 사용해서 변경
        match data_size {
            1 => 
        }
        println!("{}", value.as_());
        data.write_uint::<LittleEndian>(value.as_(), data_size).unwrap();
    }

    pub fn write_float(&mut self, addr: u32, value: f32) {
        self.check_address_space(addr);
        self.check_write_address_space(addr);

        let offset = (addr-self.v_address_range.0) as usize;
        let mut data = &mut (self.data[offset..offset+4]);
        data.write_f32::<LittleEndian>(value).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(image.read_only_range.1, 0x10000+0x6dd46);
        assert_eq!(entry_point, 0x10338);
        assert_eq!(image.data.len(), 0x7fd08+0x1e0c - 0x10000);
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
        let str_bytes = (0x625b0..0x625b0+13).map(|addr| image.read_int::<u8>(addr)).collect::<Vec<u8>>();
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

        image.write_int(0x7fd08, b'A');
        let result: char = image.read_int::<u8>(0x7fd08).into();
        assert_eq!(result, 'A');
        image.write_int(0x80000, 100u32);
        assert_eq!(image.read_int::<u32>(0x80000), 100);
        image.write_int(0x80000, -10i8);
        assert_eq!(image.read_int::<i8>(0x80000), -10);
        //image.write_float(0x80000, 3.14);
        //assert_eq!(image.read_float(0x80000), 3.14);
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

        image.write_int(0x20000, 100);
    }
}
