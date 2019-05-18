//! Simulator components for RISC-V 32I instruction set.


pub mod alu;
pub mod ca_simulator;
pub mod consts;
pub mod hazards;
pub mod instruction;
pub mod memory;
pub mod pipeline;
pub mod register;
pub mod stages;

extern crate goblin;
extern crate byteorder;
extern crate num_traits;
extern crate nix;

pub static mut PRINT_STORES: bool = false;