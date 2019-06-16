#[derive(Debug, Clone, Copy)]
pub enum Operand {
    Value(u32),
    Rob(usize),
}


impl Default for Operand {
    fn default() -> Self {
        Operand::Value(0)
    }
}
