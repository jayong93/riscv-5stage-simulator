//! Harvard architecture (separate instruction and data) memory interface.

pub mod data;
pub mod instruction;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::cmp::Ordering;
use std::io::Read;
use std::mem::size_of;
use std::ops::Range;

#[derive(Debug, Default)]
pub struct InstructionMemory {
    // virtual address range
    v_address_range: (u32, u32),
    data: Vec<u8>,
    entry_point: u32,
}

impl InstructionMemory {
    pub fn read(&self, addr: u32) -> u32 {
        if addr < self.v_address_range.0 || addr >= self.v_address_range.1 {
            panic!("{} is out of instruction address range.", addr);
        }
        let offset = (addr - self.v_address_range.0) as usize;
        let mut data = &(self.data[offset..offset+4]);
        data.read_u32::<LittleEndian>().expect("Can't read memory as u32 instruction")
    }
}

#[derive(Debug, Default)]
pub struct DataMemory {
    v_address_range: (u32, u32),
    data: Vec<u8>,
}

impl DataMemory {
    pub fn read_int<T: ReadBytesExt + num_traits::PrimInt + num_traits::FromPrimitive>(&self, addr: u32) -> T {
        if addr < self.v_address_range.0 || addr >= self.v_address_range.1 {
            panic!("{} is out of data address range.", addr);
        }
        
        let offset = (addr - self.v_address_range.0) as usize;
        let data_size = size_of::<T>() as usize;
        let mut data = &(self.data[offset..offset+data_size]);
        let val = data.read_uint::<LittleEndian>(data_size).expect("Can't read a memory as u64");
        T::from_u64(val).unwrap()
    }

    pub fn read_float(&self, addr: u32) -> f32 {
        if addr < self.v_address_range.0 || addr >= self.v_address_range.1 {
            panic!("{} is out of data address range.", addr);
        }
        let offset = (addr - self.v_address_range.0) as usize;
        let mut data = &(self.data[offset..offset+4]);
        data.read_f32::<LittleEndian>().expect("Can't read a memory as u64")
    }
}

pub fn get_image_from_elf(elf: &[u8]) -> (InstructionMemory, DataMemory) {
    let elf_struct =
        goblin::elf::Elf::parse(elf).expect("It's not elf format data.");
    elf_struct
        .program_headers
        .iter()
        .filter(|header| header.p_type == goblin::elf::program_header::PT_LOAD)
        .fold(
            (
                InstructionMemory {
                    entry_point: elf_struct.entry as u32,
                    ..Default::default()
                },
                DataMemory::default(),
            ),
            |(mut inst, mut data), header| {
                let vm_range = header.vm_range();
                if header.is_write() {
                    if data.v_address_range.0 == data.v_address_range.1 {
                        data.v_address_range.0 = vm_range.start as u32;
                        data.v_address_range.1 = vm_range.end as u32;
                    } else {
                        data.v_address_range.1 = vm_range.end as u32;
                    }
                    let old_size = data.data.len();
                    data.data.resize(header.p_memsz as usize, 0);
                    data.data[old_size..(old_size + header.p_filesz as usize)]
                        .copy_from_slice(&elf[header.file_range()]);
                } else if header.is_read() {
                    if inst.v_address_range.0 == inst.v_address_range.1 {
                        inst.v_address_range.0 = vm_range.start as u32;
                        inst.v_address_range.1 = vm_range.end as u32;
                    } else {
                        inst.v_address_range.1 = vm_range.end as u32;
                    }
                    let old_size = inst.data.len();
                    inst.data.resize(header.p_memsz as usize, 0);
                    inst.data[old_size..(old_size + header.p_filesz as usize)]
                        .copy_from_slice(&elf[header.file_range()]);
                }
                (inst, data)
            },
        )
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
        let (inst_image, data_image) = get_image_from_elf(&elf_data);
        assert_eq!(inst_image.v_address_range.0, 0x10000);
        assert_eq!(inst_image.v_address_range.1, 0x10000 + 0x6dd46);
        assert_eq!(inst_image.entry_point, 0x10338);
        assert_eq!(inst_image.data.len(), 0x6dd46);
        assert_eq!(inst_image.read(inst_image.entry_point), 0x034000ef);
        assert_eq!(data_image.v_address_range.0, 0x7fd08);
        assert_eq!(data_image.v_address_range.1, 0x7fd08 + 0x1e0c);
        assert_eq!(data_image.data.len(), 0x1e0c);
    }
}
// pub struct RawSection {
//     range: Range<u32>,
//     data: Vec<u8>,
// }

// impl RawSection {
//     pub fn read_int<T: num_traits::PrimInt>(&self, addr: u32) -> T {
//         let size = size_of::<T>();
//         let start_addr = (addr-self.range.start) as usize;
//         let mut num = &self.data[(start_addr..start_addr+size)];
//         match size {
//             1 => num.read_u8().map(|n| T::from(n).unwrap()).expect("Can't convert to byte"),
//             2 => num.read_u16::<LittleEndian>().map(|n| T::from(n).unwrap()).expect("Can't convert to half"),
//             4 => num.read_u32::<LittleEndian>().map(|n| T::from(n).unwrap()).expect("Can't convert to word"),
//             8 => num.read_u64::<LittleEndian>().map(|n| T::from(n).unwrap()).expect("Can't convert to dword"),
//             _ => unreachable!()
//         }
//     }

//     pub fn read_float(&self, addr: u32) -> f32 {
//         let start_addr = (addr-self.range.start) as usize;
//         let mut num = &self.data[(start_addr..start_addr+4)];
//         num.read_f32::<LittleEndian>().expect("Can't convert to float")
//     }

//     pub fn write_float(&mut self, addr: u32, value: f32) {
//         let start_addr = (addr - self.range.start) as usize;
//         let mut num = &mut self.data[(start_addr..start_addr+4)];
//         num.write_f32::<LittleEndian>(value).unwrap();
//     }

//     pub fn write_int<T: num_traits::PrimInt>(&mut self, addr: u32, value: T) {
//         let size = size_of::<T>();
//         let start_addr = (addr-self.range.start) as usize;
//         let mut num = &mut self.data[(start_addr..start_addr+size)];
//         match size {
//             1 => num.write_u8(value.to_u8().unwrap()),
//             2 => num.write_u16::<LittleEndian>(value.to_u16().unwrap()),
//             4 => num.write_u32::<LittleEndian>(value.to_u32().unwrap()),
//             8 => num.write_u64::<LittleEndian>(value.to_u64().unwrap()),
//             _ => unreachable!()
//         }.unwrap();
//     }
// }

// pub enum Section {
//     Execute(RawSection),
//     Normal(RawSection),
// }

// impl Section {
//     pub fn read_int<T: num_traits::PrimInt>(&self, addr: u32) -> T {
//         match self {
//             Section::Execute(s) | Section::Normal(s) => s.read_int(addr),
//         }
//     }
//     pub fn read_float(&self, addr: u32) -> f32 {
//         match self {
//             Section::Execute(s) | Section::Normal(s) => s.read_float(addr),
//         }
//     }
//     pub fn write_int<T: num_traits::PrimInt>(&mut self, addr: u32, value: T) {
//         match self {
//             Section::Execute(s) | Section::Normal(s) => s.write_int(addr, value),
//         }
//     }
//     pub fn write_float(&mut self, addr: u32, value: f32) {
//         match self {
//             Section::Execute(s) | Section::Normal(s) => s.write_float(addr, value),
//         }
//     }
// }

// pub struct ProcessMemory {
//     sections: Vec<Section>,
//     entry_point: u32,
//     // Stack 자료구조 추가
// }

// impl ProcessMemory {
//     pub fn new(mut file: impl Read) -> Self {
//         let mut file_data = Vec::new();
//         file.read_to_end(&mut file_data).expect("Can't read data");
//         let elf =
//             goblin::elf::Elf::parse(&file_data).expect("Can't parse data");
//         let sections = elf
//             .section_headers
//             .iter()
//             .filter(|header| header.is_alloc())
//             .map(|header| {
//                 if header.is_executable() {
//                     let range = header.vm_range();
//                     let data = file_data[header.file_range()].to_vec();
//                     Section::Execute(RawSection {
//                         range: (range.start as u32..range.end as u32),
//                         data,
//                     })
//                 } else if header.sh_type
//                     == goblin::elf::section_header::SHT_NOBITS
//                 {
//                     let range = header.vm_range();
//                     let mut data = Vec::new();
//                     data.resize_with(
//                         range.end - range.start,
//                         Default::default,
//                     );
//                     Section::Normal(RawSection {
//                         range: (range.start as u32..range.end as u32),
//                         data,
//                     })
//                 } else {
//                     let range = header.vm_range();
//                     let data = file_data[header.file_range()].to_vec();
//                     Section::Normal(RawSection {
//                         range: (range.start as u32..range.end as u32),
//                         data,
//                     })
//                 }
//             })
//             .collect();
//         ProcessMemory { sections, entry_point: elf.entry as u32 }
//     }

//     pub fn read_int<T: num_traits::PrimInt>(&self, addr: u32) -> T {
//         let sec = self.get_section(addr).expect("Wrong address");
//         sec.read_int::<T>(addr)
//     }

//     pub fn read_float(&self, addr: u32) -> f32 {
//         let sec = self.get_section(addr).expect("Wrong address");
//         sec.read_float(addr)
//     }

//     pub fn write_int<T: num_traits::PrimInt>(&mut self, addr: u32, value: T) {
//         let sec = self.get_section_mut(addr).expect("Wrong address");
//         sec.write_int(addr, value)
//     }

//     pub fn write_float(&mut self, addr: u32, value: f32) {
//         let sec = self.get_section_mut(addr).expect("Wrong address");
//         sec.write_float(addr,value)
//     }

//     pub fn fetch_instruction(&self, addr: u32) -> u32 {
//         let sec = self.get_section(addr);

//         match sec {
//             Ok(section) => {
//                 if addr % 4 != 0 {panic!("Not aligned address.")}
//                 if let Section::Execute(ref s) = section {
//                     s.read_int::<u32>(addr)
//                 }
//                 else {
//                     panic!("The address is not in executable instruction section.")
//                 }
//             },
//             Err(_) => panic!("The address is not in process's memory area."),
//         }
//     }

//     fn get_section(&self, addr: u32) -> Result<&Section, usize> {
//         let sec = self.find_section_index(addr);
//         sec.map(|idx| {
//             &self.sections[idx]
//         })
//     }

//     fn get_section_mut(&mut self, addr: u32) -> Result<&mut Section, usize> {
//         let sec = self.find_section_index(addr);
//         sec.map(move |idx| {
//             &mut self.sections[idx]
//         })
//     }

//     fn find_section_index(&self, addr: u32) -> Result<usize, usize> {
//         self.sections.binary_search_by(|section| {
//             let range = match section {
//                 Section::Execute(s) | Section::Normal(s) => &s.range,
//             };
//             if addr < range.start {
//                 Ordering::Less
//             } else if addr >= range.end {
//                 Ordering::Greater
//             } else {
//                 Ordering::Equal
//             }
//         })
//     }
// }
