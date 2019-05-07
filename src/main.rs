//! A 5-stage pipelining RISC-V 32I simulator.


extern crate env_logger;
extern crate riscv_5stage_simulator;

use std::env;
use std::fs::File;
use std::io::prelude::*;
use riscv_5stage_simulator::memory::ProcessMemory;
use riscv_5stage_simulator::pipeline::Pipeline;

fn main() {
    env_logger::init().unwrap();

    let args: Vec<String> = env::args().collect();
    let program_name = &args[0];
    let mut f_data = Vec::new();
    let process_image;
    let elf;

    if let Some(filename) = args.get(1) {
        let mut f = File::open(filename).expect("error opening file");
        f.read_to_end(&mut f_data).expect("Can't read from a file");
        elf = goblin::elf::Elf::parse(&f_data).expect("It's not a elf binary file");
        process_image = ProcessMemory::new(&elf, &f_data);
    } else {
        println!("Usage: {} <filename>", program_name);
        std::process::exit(1);
    }

    let mut pipeline = Pipeline::new(dbg!(elf.entry) as u32, process_image);

    let mut clock = 1..;
    loop {
        println!("Clock #{} | pc: {:x} | {:?}",
                 clock.next().unwrap(),
                 pipeline.mem_wb.pc,
                 pipeline.mem_wb.inst);
        pipeline.run_clock();
        if pipeline.is_finished { break }
    }
}
