//! Per-compiler debug-info parsers. `crate::source_map` (solc) and `dwarf`
//! (solx) both produce the same [`crate::build_model::Instruction`] vector, so
//! the rest of the stack-trace pipeline stays compiler-agnostic.
//!
//! The [`CompilerArtifact`] trait is the seam: each compiler-specific bytecode
//! type knows how to decode its own debug-info AND advertises its
//! stack-trace strategy through [`CompilerArtifact::trace_strategy`], so
//! callers dispatch polymorphically over both concerns.

use std::{collections::HashMap, sync::Arc};

use auto_impl::auto_impl;
use indexmap::IndexMap;

use crate::{
    artifacts::{
        CompilerOutputSource, ImmutableReference, LinkReference, SolcBytecode, SolxBytecode,
    },
    build_model::{BuildModel, Instruction},
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

    /// Decode this artifact's debug-info into the canonical
    /// [`Instruction`] vector consumed by the stack-trace pipeline.
    fn decode_instructions(
        &self,
        normalized_code: &[u8],
        build_model: &Arc<BuildModel>,
        is_deployment: bool,
    ) -> anyhow::Result<Vec<Instruction>>;

    /// Compiler-specific stack-trace strategy used by the error inferrer's
    /// heuristics.
    fn trace_strategy(&self) -> &'static dyn TraceStrategy;

    /// Per-file AST `src` spans (`file_id` → sorted `(offset, length)`) this
    /// artifact's debug-info decoder needs from the compilation output's
    /// ASTs. The sourceMap decoder resolves spans directly and collects
    /// none; the DWARF decoder derives `SourceLocation.length` from them.
    fn collect_ast_spans(
        &self,
        sources: &IndexMap<String, CompilerOutputSource>,
    ) -> HashMap<u32, Vec<(u32, u32)>>;
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

    fn trace_strategy(&self) -> &'static dyn TraceStrategy {
        &SolcTraceStrategy
    }

    fn collect_ast_spans(
        &self,
        _sources: &IndexMap<String, CompilerOutputSource>,
    ) -> HashMap<u32, Vec<(u32, u32)>> {
        HashMap::new()
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

    fn trace_strategy(&self) -> &'static dyn TraceStrategy {
        &SolxTraceStrategy
    }

    fn collect_ast_spans(
        &self,
        sources: &IndexMap<String, CompilerOutputSource>,
    ) -> HashMap<u32, Vec<(u32, u32)>> {
        let mut spans: HashMap<u32, Vec<(u32, u32)>> = HashMap::new();
        for source in sources.values() {
            collect_node_spans(&source.ast, &mut spans);
        }
        // Sorted so `BuildModel::smallest_enclosing_span` can scan in order
        // and break early.
        for file_spans in spans.values_mut() {
            file_spans.sort_unstable();
            file_spans.dedup();
        }
        spans
    }
}

/// Walk an AST subtree and append every node's `src` span keyed by file ID.
fn collect_node_spans(node: &serde_json::Value, out: &mut HashMap<u32, Vec<(u32, u32)>>) {
    if let Some(src) = node.get("src").and_then(serde_json::Value::as_str)
        && let Some((offset, length, file_id)) = parse_src(src)
    {
        out.entry(file_id).or_default().push((offset, length));
    }
    if let Some(obj) = node.as_object() {
        for value in obj.values() {
            collect_node_spans(value, out);
        }
    } else if let Some(arr) = node.as_array() {
        for value in arr {
            collect_node_spans(value, out);
        }
    }
}

/// Parse `"offset:length:fileIndex"` into `(offset, length, file_id)`.
fn parse_src(src: &str) -> Option<(u32, u32, u32)> {
    let mut parts = src.splitn(3, ':');
    let offset = parts.next()?.parse::<u32>().ok()?;
    let length = parts.next()?.parse::<u32>().ok()?;
    let file_id = parts.next()?.parse::<u32>().ok()?;
    Some((offset, length, file_id))
}
