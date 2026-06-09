//! Per-compiler debug-info parsers. `crate::source_map` (solc) and `dwarf`
//! (solx) both produce the same [`crate::build_model::Instruction`] vector, so
//! the rest of the stack-trace pipeline stays compiler-agnostic.
//!
//! The [`CompilerArtifact`] trait is the seam: each compiler-specific bytecode
//! type knows how to decode its own debug-info, and callers operate against
//! the trait so they can stay oblivious to which compiler produced the input.
//!
//! [`SolxBuildModelExt`] segregates the solx-only methods that the DWARF
//! parser needs on [`BuildModel`]. Keeping them behind a trait — sealed so
//! external crates can't impl it — prevents solc-only call paths from
//! accidentally depending on solx-specific state.

use std::{any::Any, collections::HashMap, sync::Arc};

use auto_impl::auto_impl;

use crate::{
    artifacts::{CompilerType, ImmutableReference, LinkReference, SolcBytecode, SolxBytecode},
    build_model::{BuildModel, Instruction},
};

pub(crate) mod dwarf;

mod sealed {
    pub trait Sealed {}
    impl Sealed for crate::build_model::BuildModel {}
}

/// Solx-only [`BuildModel`] accessors. Imported only by the DWARF decode
/// path; solc-only call sites stay oblivious to these helpers.
pub trait SolxBuildModelExt: sealed::Sealed {
    /// Reverse-index of `file_id_to_source_file` keyed by source name —
    /// the DWARF parser uses this to resolve a DWARF file string back to
    /// the [`BuildModel`]'s `file_id`.
    fn name_to_file_id(&self) -> &HashMap<String, u32>;

    /// Smallest (leafmost) AST `(offset, length)` span containing `offset` —
    /// the DWARF parser uses this to widen a zero-length `(file, line)`
    /// hit into the surrounding AST node's span for the renderer.
    fn smallest_enclosing_span(&self, file_id: u32, offset: u32) -> Option<(u32, u32)>;
}

impl SolxBuildModelExt for BuildModel {
    fn name_to_file_id(&self) -> &HashMap<String, u32> {
        self.name_to_file_id.get_or_init(|| {
            self.file_id_to_source_file
                .iter()
                .map(|(id, file)| (file.read().source_name.clone(), *id))
                .collect()
        })
    }

    fn smallest_enclosing_span(&self, file_id: u32, offset: u32) -> Option<(u32, u32)> {
        let spans = self.ast_spans.get(&file_id)?;
        let mut best: Option<(u32, u32)> = None;
        for &(span_offset, span_length) in spans {
            if span_offset > offset {
                break;
            }
            if offset < span_offset.saturating_add(span_length)
                && best.is_none_or(|(_, best_len)| span_length < best_len)
            {
                best = Some((span_offset, span_length));
            }
        }
        best
    }
}

/// Per-compiler bytecode artifact. The behaviour contract that the
/// stack-trace pipeline programs against — concrete types
/// ([`SolcBytecode`], [`SolxBytecode`]) hold the data, the trait carries
/// the operations.
///
/// Used through `Arc<dyn CompilerArtifact>` (see
/// [`crate::artifacts::CompilerArtifact`]) so the pipeline dispatches
/// dynamically and stays open to additional compiler implementations.
#[auto_impl(&, Box)]
pub trait CompilerArtifact: std::fmt::Debug {
    /// Producing compiler, derived from the concrete type implementing this
    /// trait.
    fn compiler_type(&self) -> CompilerType;

    /// Hex-encoded creation- or runtime-bytecode `object` from the
    /// Standard JSON output.
    fn object(&self) -> &str;

    /// Disassembled opcode text from the Standard JSON output.
    fn opcodes(&self) -> &str;

    /// Library link references (source → library name → positions).
    fn link_references(&self) -> &HashMap<String, HashMap<String, Vec<LinkReference>>>;

    /// Immutable-variable references emitted by the compiler, if any.
    fn immutable_references(&self) -> Option<&HashMap<String, Vec<ImmutableReference>>>;

    /// Decode this artifact's debug-info into the canonical
    /// [`Instruction`] vector consumed by the stack-trace pipeline.
    fn decode_instructions(
        &self,
        normalized_code: &[u8],
        build_model: &Arc<BuildModel>,
        is_deployment: bool,
    ) -> anyhow::Result<Vec<Instruction>>;

    /// Downcast hook so tests can recover the concrete type.
    fn as_any(&self) -> &dyn Any;
}

impl CompilerArtifact for SolcBytecode {
    fn compiler_type(&self) -> CompilerType {
        CompilerType::Solc
    }

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

    fn decode_instructions(
        &self,
        normalized_code: &[u8],
        build_model: &Arc<BuildModel>,
        is_deployment: bool,
    ) -> anyhow::Result<Vec<Instruction>> {
        crate::source_map::decode_instructions(
            normalized_code,
            &self.source_map,
            build_model,
            is_deployment,
        )
        .map_err(Into::into)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl CompilerArtifact for SolxBytecode {
    fn compiler_type(&self) -> CompilerType {
        CompilerType::Solx
    }

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

    fn decode_instructions(
        &self,
        normalized_code: &[u8],
        build_model: &Arc<BuildModel>,
        is_deployment: bool,
    ) -> anyhow::Result<Vec<Instruction>> {
        dwarf::decode_instructions(
            normalized_code,
            &self.debug_info,
            build_model,
            is_deployment,
        )
        .map_err(Into::into)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
