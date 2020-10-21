# RISC-V 5-Stage Pipeline Simulator 

RISC-V 5-Stage Pipeline Simulator is a 32-bit integer instruction
set architecture (ISA) and pipelining RISC-V simulator written in
Rust. The simulator is based on the design in the book Computer
Organization and Design RISC-V Edition by Patterson and Hennessy.


## Quickstart

### General usage:

1) Follow instructions at [rustup.rs](https://rustup.rs/) to install Rust stable for your platform.  
    But, It only support linux and 32bit riscv binary, so you have to install `<channel>-<some 32bit arch>-unknown-linux-gnu` rust toolchain and gcc-multilib for 32bit build.
    For example, if your cpu architecture is x86_64(aka amd64), you would better do:
    ```bash
    rustup toolchain install stable-i686-unknown-linux-gnu
    ```
    You can get full list of toolchain target with:
    ```bash
    rustup target list
    ```
2) Run with elf binary :
    ```bash
    cd riscv-5stage-simulator
    cargo +<toolchain-you-installed-above> build # build the simulator
    target/debug/casim <path-for-elf>
    ```

    For more options:
    ```bash
    target/debug/casim --help
    ```

## Licence

Copyright 2017 Douglas Anderson <douglas.anderson-1@colorado.edu>, Jaeyong Choi <jayong93@gmail.com>. Released
under GPL 3 _except for the 3 disassembly files in tests/ which are copyright
their respective authors and not covered under this license._
