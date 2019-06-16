//! A 5-stage pipelining RISC-V 32I simulator.

extern crate riscv_5stage_simulator;
extern crate structopt;
extern crate lazy_static;

use riscv_5stage_simulator::memory::ProcessMemory;
use riscv_5stage_simulator::pipeline::Pipeline;
use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;
use structopt::StructOpt;
use lazy_static::lazy_static;

#[derive(StructOpt, Debug)]
#[structopt(name = "casim")]
struct Opt {
    #[structopt(parse(from_os_str))]
    elf_binary: PathBuf,
    #[structopt(long = "print-steps")]
    /// Prints clocks and instruction infomations when the instruction is write-backed
    print_steps: bool,
    #[structopt(long = "print-debug-info")]
    /// Prints informations for debugging
    print_debug_info: bool,
}

lazy_static! {
    static ref OPTS: Opt = Opt::from_args();
}

fn main() {
    unsafe{ riscv_5stage_simulator::PRINT_DEBUG_INFO = OPTS.print_debug_info };

    let mut f_data = Vec::new();
    let process_image;
    let elf;

    let mut f = File::open(&OPTS.elf_binary).expect("error opening file");
    f.read_to_end(&mut f_data).expect("Can't read from a file");
    elf = goblin::elf::Elf::parse(&f_data).expect("It's not a elf binary file");
    process_image = ProcessMemory::new(&elf, &f_data, OPTS.elf_binary.to_str().unwrap());

    let mut pipeline = Pipeline::new(elf.entry as u32, process_image);

    let mut clock_it = 1..;
    loop {
        let clock = clock_it.next().unwrap();
        let (retired_robs, is_finished) = pipeline.run_clock();
        if OPTS.print_steps {
            for (_old_index, entry) in retired_robs {
                eprint!(
                    "Clock #{} | pc: {:x} | val: {:08x} | inst: {:?} | fields: {}",
                    clock,
                    entry.pc,
                    entry.inst.value,
                    entry.inst.function,
                    entry.inst.fields,
                );
                if OPTS.print_debug_info {
                    eprint!(" | regs: {}", pipeline.reg)
                }
                eprintln!("");
            }
            if is_finished {
                break;
            }
        }
    }
}
