//! Per-compiler debug-info parsers. [`crate::source_map`] (solc) and [`dwarf`]
//! (solx) both produce the same [`crate::build_model::Instruction`] vector, so
//! the rest of the stack-trace pipeline stays compiler-agnostic.

pub(crate) mod dwarf;
