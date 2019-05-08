# RISC-V 5-Stage Pipeline Simulator 

RISC-V 5-Stage Pipeline Simulator is a 32-bit integer instruction
set architecture (ISA) and pipelining RISC-V simulator written in
Rust. The simulator is based on the design in the book Computer
Organization and Design RISC-V Edition by Patterson and Hennessy.


## Quickstart

### General usage:

 1) Follow instructions at [rustup.rs](https://rustup.rs/) to install Rust stable for your platform.
 2) Run with elf binary :
    ```bash
    cd riscv-5stage-simulator
    cargo run path-for-elf
    ```

## Licence

Copyright 2017 Douglas Anderson <douglas.anderson-1@colorado.edu>. Released
under GPL 3 _except for the 3 disassembly files in tests/ which are copyright
their respective authors and not covered under this license._
