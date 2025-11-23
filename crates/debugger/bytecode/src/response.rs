use crate::shared::Breakpoint;

pub enum Response {
    SetInstructionBreakpoints {
        /// Information about the breakpoints. The array elements correspond to
        /// the elements of the `breakpoints` array.
        breakpoints: Vec<Breakpoint>,
    },
}
