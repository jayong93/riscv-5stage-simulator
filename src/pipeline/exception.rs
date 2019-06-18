#[derive(Debug, Copy, Clone)]
pub enum Exception {
    WritingToInvalidMemory(u32),
    WritingToReadOnlyMemory(u32),
    SyscallNotImpl(u32),
    FailCallingSyscall(u32),
}
