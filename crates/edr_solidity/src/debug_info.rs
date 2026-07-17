//! Per-compiler debug-info parsers. `crate::artifacts::solc::source_map` (solc)
//! and `dwarf` (solx) both produce the same [`crate::build_model::Instruction`]
//! vector, so the rest of the stack-trace pipeline stays compiler-agnostic.
//!
//! The [`CompilerArtifact`] trait is the seam: each compiler-specific bytecode
//! type knows how to decode its own debug-info AND advertises its
//! stack-trace strategy through [`CompilerArtifact::trace_strategy`], so
//! callers dispatch polymorphically over both concerns.

use std::collections::HashMap;

use crate::{
    artifacts::{CompilerArtifact, ImmutableReference, LinkReference, SolcBytecode, SolxBytecode},
    trace_strategy::{SolcTraceStrategy, SolxTraceStrategy, TraceStrategy},
};

pub(crate) mod dwarf;

impl CompilerArtifact for SolcBytecode {
    fn object(&self) -> &str {
        &self.object
    }

    fn opcodes(&self) -> &str {
        &self.opcodes
    }

    fn link_references(&self) -> &HashMap<String, HashMap<String, Vec<LinkReference>>> {
        &self.link_references
    }

    fn immutable_references(&self) -> Option<&HashMap<String, Vec<ImmutableReference>>> {
        self.immutable_references.as_ref()
    }

    fn trace_strategy(&self) -> &'static dyn TraceStrategy {
        &SolcTraceStrategy
    }
}

impl CompilerArtifact for SolxBytecode {
    fn object(&self) -> &str {
        &self.object
    }

    fn opcodes(&self) -> &str {
        &self.opcodes
    }

    fn link_references(&self) -> &HashMap<String, HashMap<String, Vec<LinkReference>>> {
        &self.link_references
    }

    fn immutable_references(&self) -> Option<&HashMap<String, Vec<ImmutableReference>>> {
        self.immutable_references.as_ref()
    }

    fn trace_strategy(&self) -> &'static dyn TraceStrategy {
        &SolxTraceStrategy
    }
}
