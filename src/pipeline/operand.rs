#[derive(Debug, Clone, Copy)]
pub enum Operand {
    Value(u32),
    Rob(usize),
    None,
}

impl Default for Operand {
    fn default() -> Self {
        Operand::None
    }
}
