//! Harvard architecture (separate instruction and data) memory interface.

pub mod data;
pub mod instruction;

use std::cmp::Ordering;
use std::io::Read;
use std::ops::Range;
use byteorder::ReadBytesExt;

pub struct RawSection {
    range: Range<u32>,
    data: Vec<u8>,
}

pub enum Section {
    Execute(RawSection),
    Normal(RawSection),
}

pub struct ProcessMemory {
    sections: Vec<Section>,
}

impl ProcessMemory {
    pub fn new(mut file: impl Read) -> Self {
        let mut file_data = Vec::new();
        file.read_to_end(&mut file_data).expect("Can't read data");
        let elf =
            goblin::elf::Elf::parse(&file_data).expect("Can't parse data");
        let sections = elf
            .section_headers
            .iter()
            .filter(|header| header.is_alloc())
            .map(|header| {
                if header.is_executable() {
                    let range = header.vm_range();
                    let data = file_data[header.file_range()].to_vec();
                    Section::Execute(RawSection {
                        range: (range.start as u32..range.end as u32),
                        data,
                    })
                } else if header.sh_type
                    == goblin::elf::section_header::SHT_NOBITS
                {
                    let range = header.vm_range();
                    let mut data = Vec::new();
                    data.resize_with(
                        range.end - range.start,
                        Default::default,
                    );
                    Section::Normal(RawSection {
                        range: (range.start as u32..range.end as u32),
                        data,
                    })
                } else {
                    let range = header.vm_range();
                    let data = file_data[header.file_range()].to_vec();
                    Section::Normal(RawSection {
                        range: (range.start as u32..range.end as u32),
                        data,
                    })
                }
            })
            .collect();
        ProcessMemory { sections }
    }

    pub fn read_int(&self, addr: u32, size: u8) -> u32 {
        let sec = self.get_section(addr);
        match (sec, size) {
            (Ok(sec), 1) => ,
            (Ok(sec), 2) => ,
            (Ok(sec), 4) => ,
            (Ok(_), _) => panic!("Wrong integer size.")
            (Err(_), _) => panic!("The address is not in valid process memory")
        }
    }

    pub fn read_float(&self, addr: u32) -> f32 {
        unimplemented!()
    }

    pub fn write_int(&mut self, addr: u32, size: u8, value: u32) {
        unimplemented!()
    }

    pub fn write_float(&mut self, addr: u32, value: f32) {
        unimplemented!()
    }

    pub fn fetch_instruction(&self, addr: u32) -> u32 {
        let sec = self.get_section(addr);

        match sec {
            Ok(section) => {
                if addr % 4 != 0 {panic!("Not aligned address.")}
                if let Section::Execute(ref s) = section {
                    (&s.data[addr as usize..(addr+4) as usize]).read_u32::<byteorder::LittleEndian>().expect("Can't read as u32")
                }
                else {
                    panic!("The address is not in executable instruction section.")
                }
            },
            Err(_) => panic!("The address is not in process's memory area."),
        }
    }

    fn get_section(&self, addr: u32) -> Result<&Section, usize> {
        let sec = self.sections.binary_search_by(|section| {
            let range = match section {
                Section::Execute(s) | Section::Normal(s) => &s.range,
            };
            if addr < range.start {
                Ordering::Less
            } else if addr >= range.end {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        });

        sec.map(|idx| {
            &self.sections[idx]
        })
    }
}
