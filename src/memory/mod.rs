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
    pub fn new(
        elf_struct: &goblin::elf::Elf,
        elf_data: &[u8],
        program_name: &str,
    ) -> Self {
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
                    memory.data.resize(vm_range.start, 0);
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
        self.stack.resize(stack_size as usize, 0);
        self.stack_range = (0u32.wrapping_sub(stack_size), 0);

        let sp = 0u32;
        // push program headers, argument strings
        let (sp, header_num) =
            self.push_program_headers(program_headers, sp);
        let header_addr = sp;
        let sp = self.push_program_name(program_name, sp);
        let program_name_addr = sp;
        // push AUX_NULL Aux vec
        let sp = {
            let sp = sp.wrapping_sub(size_of::<AuxVec>() as u32);
            self.write(sp, AuxVec::new(0, 0));
            sp
        };
        // push Aux vecs
        let sp = self.push_aux_vecs(header_addr, header_num, program_name_addr, entry_point, sp);
        let sp = [0u32, 0u32, program_name_addr, 1u32].iter().fold(
            sp,
            |sp, val| {
                let sp = sp.wrapping_sub(size_of::<u32>() as u32);
                self.write(sp, *val);
                sp
            },
        );

        self.stack_pointer_init = sp;
    }

    fn push_program_headers(
        &mut self,
        headers: &[Elf32ProgramHeader],
        sp: u32,
    ) -> (u32, u32) {
        headers.iter().fold((sp, 0), |(sp, num), header| {
            let sp = sp.wrapping_sub(size_of::<Elf32ProgramHeader>() as u32);
            self.write(sp, *header);
            (sp, num+1)
        })
    }

    fn push_program_name(&mut self, name: &str, sp: u32) -> u32 {
        let name_cstring = std::ffi::CString::new(name.to_owned()).unwrap();
        let name_bytes = name_cstring.as_bytes_with_nul();
        let sp = sp.wrapping_sub(name_bytes.len() as u32);
        self.write_slice(sp, name_bytes);
        sp
    }

    fn push_aux_vecs(
        &mut self,
        header_addr: u32,
        header_num: u32,
        info_block_addr: u32,
        entry_point: u32,
        sp: u32,
    ) -> u32 {
        use self::consts::*;
        let aux_vecs : Vec<_> = [
            (AT_ENTRY, entry_point),
            (AT_PHNUM, header_num),
            (AT_PHENT, size_of::<Elf32ProgramHeader>() as u32),
            (AT_PHDR, header_addr),
            (AT_PAGESZ, 0),
            (AT_SECURE, 0),
            (AT_RANDOM, info_block_addr),
            (AT_NULL, 0),
        ].iter().map(|(t, d)| AuxVec::new(*t, *d)).collect();
        let sp = sp.wrapping_sub((size_of::<AuxVec>() * aux_vecs.len()) as u32);
        self.write_slice(sp, aux_vecs.as_slice());
        sp
    }

    fn check_address_space(&self, addr: u32) {
        if addr < self.v_address_range.0
            || (addr >= self.v_address_range.1 && addr < self.stack_range.0)
        {
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

    pub fn read<T: Copy>(&self, addr: u32) -> T {
        let data_size = size_of::<T>() as usize;
        let data_ptr = self.read_bytes(addr, data_size).as_ptr() as *const T;
        unsafe { *data_ptr }
    }

    pub fn read_bytes(&self, addr: u32, size: usize) -> &[u8] {
        self.check_address_space(addr);

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
        self.check_address_space(addr);

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

    pub fn write<T>(&mut self, addr: u32, value: T) {
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

    pub fn write_slice<T>(&mut self, addr: u32, value: &[T]) {
        self.check_address_space(addr);
        self.check_write_address_space(addr);

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
        let byte_slice = unsafe {
            std::slice::from_raw_parts(ptr, value.len() * data_size)
        };
        data.copy_from_slice(byte_slice);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    const TEST_BINARY: &'static str = "tests/hello";

    #[test]
    fn test_reading_elf() {
        let mut elf_data = Vec::new();
        let _ = std::fs::File::open(TEST_BINARY)
            .unwrap()
            .read_to_end(&mut elf_data)
            .unwrap();
        let elf = goblin::elf::Elf::parse(&elf_data).unwrap();
        let image = ProcessMemory::new(&elf, &elf_data, TEST_BINARY);
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
        let _ = std::fs::File::open(TEST_BINARY)
            .unwrap()
            .read_to_end(&mut elf_data)
            .unwrap();
        let elf = goblin::elf::Elf::parse(&elf_data).unwrap();
        let image = ProcessMemory::new(&elf, &elf_data, TEST_BINARY);
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
        let _ = std::fs::File::open(TEST_BINARY)
            .unwrap()
            .read_to_end(&mut elf_data)
            .unwrap();
        let elf = goblin::elf::Elf::parse(&elf_data).unwrap();
        let mut image = ProcessMemory::new(&elf, &elf_data, TEST_BINARY);

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
        let _ = std::fs::File::open(TEST_BINARY)
            .unwrap()
            .read_to_end(&mut elf_data)
            .unwrap();
        let elf = goblin::elf::Elf::parse(&elf_data).unwrap();
        let mut image = ProcessMemory::new(&elf, &elf_data, TEST_BINARY);

        image.write(0x20000, 100);
    }

    #[test]
    fn test_rw_stack() {
        let mut elf_data = Vec::new();
        let _ = std::fs::File::open(TEST_BINARY)
            .unwrap()
            .read_to_end(&mut elf_data)
            .unwrap();
        let elf = goblin::elf::Elf::parse(&elf_data).unwrap();
        let mut image = ProcessMemory::new(&elf, &elf_data, TEST_BINARY);

        image.write(0u32.overflowing_sub(10).0, 911u32);
        assert_eq!(image.read::<u32>(0u32.overflowing_sub(10).0), 911u32);
    }
}
