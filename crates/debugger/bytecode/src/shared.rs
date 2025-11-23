pub struct Breakpoint {
    /// The identifier for the breakpoint. It is needed if breakpoint events are
    /// used to update or remove breakpoints.
    pub id: Option<usize>,
}

pub struct InstructionBreakpoint {
    /// The instruction reference of the breakpoint.
    /// This should be a memory or instruction pointer reference from an
    /// `EvaluateResponse`, `Variable`, `StackFrame`, `GotoTarget`, or
    /// `Breakpoint`.
    pub instruction_reference: String,
    /// The offset from the instruction reference in bytes.
    /// This can be negative.
    pub offset: Option<isize>,
}
