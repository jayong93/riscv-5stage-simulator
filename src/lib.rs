//! Simulator components for RISC-V 32I instruction set.


pub mod alu;
pub mod consts;
pub mod instruction;
pub mod memory;
pub mod pipeline;
pub mod register;

extern crate goblin;
extern crate byteorder;
extern crate num_traits;
extern crate nix;

pub static mut PRINT_STEPS: bool = false;
pub static mut PRINT_DEBUG_INFO: bool = false;