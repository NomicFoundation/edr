//! Per-compiler debug-info parsers. `crate::source_map` (solc) and `dwarf`
//! (solx) both produce the same [`crate::build_model::Instruction`] vector, so
//! the rest of the stack-trace pipeline stays compiler-agnostic.
//!
//! The [`CompilerArtifact`] trait is the seam: each compiler-specific bytecode
//! type knows how to decode its own debug-info AND advertises its
//! stack-trace strategy through [`CompilerArtifact::trace_strategy`], so
//! callers dispatch polymorphically over both concerns.

use std::collections::HashMap;

use auto_impl::auto_impl;

use crate::{
    artifacts::{ImmutableReference, LinkReference, SolcBytecode, SolxBytecode},
    trace_strategy::{SolcTraceStrategy, SolxTraceStrategy, TraceStrategy},
};

pub(crate) mod dwarf;

/// Per-compiler bytecode artifact. The behaviour contract that the
/// stack-trace pipeline programs against — concrete types
/// ([`SolcBytecode`], [`SolxBytecode`]) hold the data, the trait carries
/// the operations.
///
/// Used through `Box<dyn CompilerArtifact>` so the pipeline dispatches
/// dynamically and stays open to additional compiler implementations.
#[auto_impl(&, Box)]
pub trait CompilerArtifact: std::fmt::Debug + 'static {
    /// Hex-encoded creation- or runtime-bytecode `object` from the
    /// Standard JSON output.
    fn object(&self) -> &str;

    /// Disassembled opcode text from the Standard JSON output.
    fn opcodes(&self) -> &str;

    /// Library link references (source → library name → positions).
    fn link_references(&self) -> &HashMap<String, HashMap<String, Vec<LinkReference>>>;

    /// Immutable-variable references emitted by the compiler, if any.
    fn immutable_references(&self) -> Option<&HashMap<String, Vec<ImmutableReference>>>;

    /// Compiler-specific stack-trace strategy used by the error inferrer's
    /// heuristics.
    fn trace_strategy(&self) -> &'static dyn TraceStrategy;
}

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
