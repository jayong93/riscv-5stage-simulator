use goblin::elf;
use byteorder::*;

#[derive(Debug, Default)]
pub struct Section {
    address: usize,
    size: usize,
    data: Vec<u32>
}

fn text_section(elf: &elf::Elf, data: &Vec<u8>) -> Option<Section> {
    elf.section_headers.iter()
        .inspect(|header| {eprintln!("{:?}", header);})
        .filter(|header| header.sh_type == 1)
        .filter(|header| elf.shdr_strtab.get(header.sh_name)
                .filter(|res| {
                    res.as_ref()
                        .ok()
                        .filter(|name| {
                            eprintln!("{}", name);
                            name == &&".text"
                        })
                        .is_some()
                })
                .is_some())
        .map(|header| Section {
            address: header.sh_addr as usize,
            size: header.sh_size as usize,
            data: data.as_slice().chunks_exact(4).map(|chunk| {
                if elf.little_endian {
                    chunk.read_u32::<LittleEndian>().unwrap()
                } else {
                    chunk.read_u32::<BigEndian>().unwrap()
                }
            }).collect()
        }).take(1).collect::<Vec<_>>().pop()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_section_address_test() {
        use std::fs::*;
        use std::io::{Read};

        let mut file = File::open(concat!(env!("CARGO_MANIFEST_DIR"), "/src/tests/hello")).unwrap();
        let mut bytes: Vec<u8> = Vec::new();
        file.read_to_end(&mut bytes);
        let elf = elf::Elf::parse(&bytes).unwrap();
        let t_section = text_section(&elf, &bytes).unwrap();
        assert_eq!(t_section.address, 0x400680);
        assert_eq!(t_section.size, 0x1e2);
    }
}
