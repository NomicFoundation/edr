pub enum StoppedReason {
    InstructionBreakpoint,
}

pub enum Event {
    Stopped {
        /// The reason for the event.
        reason: StoppedReason,
        /// Ids of the breakpoints that triggered the event. In most cases there
        /// is only a single breakpoint but here are some examples for multiple
        /// breakpoints:
        /// - Different types of breakpoints map to the same location.
        /// - Multiple source breakpoints get collapsed to the same instruction
        ///   by the compiler/runtime.
        /// - Multiple function breakpoints with different function names map to
        ///   the same location.
        hit_breakpoint_ids: Option<Vec<usize>>,
    },
}
