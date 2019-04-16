use byteorder::*;
use goblin::elf;

#[derive(Debug, Default)]
pub struct Section {
    base_addr: usize,
    size: usize,
    data: Vec<u8>,
}

#[allow(dead_code)]
fn extract_section(
    elf: &elf::Elf,
    _data: &Vec<u8>,
    name: &str,
) -> Option<Section> {
    elf.section_headers
        .iter()
        .filter(|header| header.sh_type == 1)
        .inspect(|header| {
            eprintln!("{:?}", header);
        })
        .filter(|header| {
            elf.shdr_strtab
                .get(header.sh_name)
                .filter(|res| {
                    res.as_ref()
                        .ok()
                        .filter(|n| {
                            eprintln!("{}", n);
                            **n == name
                        })
                        .is_some()
                })
                .is_some()
        })
        .map(|header| {
            let base_addr = header.sh_addr as usize;
            let size = header.sh_size as usize;
            Section {
                base_addr,
                size,
                ..Default::default()
            }
        })
        .take(1)
        .collect::<Vec<_>>()
        .pop()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_section_address_test() {
        use std::fs::*;
        use std::io::Read;

        let mut file = File::open(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/tests/hello"
        ))
        .unwrap();
        let mut bytes: Vec<u8> = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        let elf = elf::Elf::parse(&bytes).unwrap();
        let t_section = extract_section(&elf, &bytes, ".text").unwrap();
        assert_eq!(t_section.base_addr, 0x10114);
        assert_eq!(t_section.size, 0x51830);
    }
}
