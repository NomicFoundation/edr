use crate::shared::InstructionBreakpoint;

pub enum Request {
    SetInstructionBreakpoints {
        /// The instruction references of the breakpoints
        breakpoints: Vec<InstructionBreakpoint>,
    },
}
