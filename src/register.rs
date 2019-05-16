
//! 32-bit register and RV32I register file.

use std::fmt;

/// A complete RV32I register file.
///
/// Holds 32 general purpose registers and a program counter register.
#[derive(Debug)]
pub struct RegisterFile {
    pub pc: Register,
    pub gpr: [Register; 32],
    pub fpr: [Register; 32],
}

impl fmt::Display for RegisterFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[ ")?;
        for (i, reg) in self.gpr.as_ref().iter().enumerate() {
            write!(f, "#{}={:x}, ", i, reg.read())?;
        }
        write!(f, "]")
    }
}

impl RegisterFile {
    /// Constructs a new `RegisterFile`.
    pub fn new(pc: u32, stack_pointer: u32) -> RegisterFile {
        let mut reg_file = RegisterFile {
            pc: Register::new(pc, true),
            gpr: [Register::new(0, true); 32],
            fpr: [Register::new(0, true); 32],
        };
        reg_file.gpr[0] = Register::new(0, false); // reinit x0 as read-only
        reg_file.gpr[2] = Register::new(stack_pointer, true);

        reg_file
    }
}

/// A write-protectable register.
#[derive(Clone, Copy, Debug)]
pub struct Register {
    /// The current register value.
    value: u32,

    /// If false, writing to the register has no effect.
    is_writable: bool,
}

impl Register {
    /// Constructs a new `Register`.
    pub fn new(value: u32, is_writable: bool) -> Register {
        Register { value, is_writable }
    }

    /// Reads the register's value.
    pub fn read(&self) -> u32 {
        self.value
    }

    /// Writes `value` to the register if it's writable, otherwise no effect.
    pub fn write<T>(&mut self, value: T) {
        if std::mem::size_of::<T>() > 4 {
            panic!("Can't write a value with size bigger than 4 to register")
        }
        if self.is_writable {
            self.value = unsafe { *(&value as *const T as *const u32) };
        }
    }
}
