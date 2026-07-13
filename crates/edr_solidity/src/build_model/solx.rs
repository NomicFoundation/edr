//! solx-specific build model types.

use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

use indexmap::IndexMap;
use parking_lot::RwLock;

use super::{
    collect_compiled_contracts_and_files, BuildModel, CompiledContractsAndFiles, Contract,
    SourceFile,
};
use crate::{
    artifacts::{collect_ast_spans, CompilerInput, CompilerOutput, SolxBytecode},
    debug_info::dwarf,
};

/// A resolved build model from a solx Solidity compiler standard JSON output.
#[derive(Debug)]
pub struct SolxBuildModel {
    /// Per-file AST `src` spans (`file_id` → sorted `(offset, length)`).
    /// The DWARF parser uses this to derive `SourceLocation.length` from a
    /// `(file, line, column)` triple.
    ast_spans: HashMap<u32, Vec<(u32, u32)>>,
    // TODO https://github.com/NomicFoundation/edr/issues/759
    /// Maps the contract ID to the contract.
    contract_id_to_contract: IndexMap<u32, Arc<RwLock<Contract>>>,
    /// Maps the file ID to the source file.
    file_id_to_source_file: Arc<HashMap<u32, Arc<RwLock<SourceFile>>>>,
    /// Lazy reverse-index `source_name` → `file_id`. See
    /// [`Self::name_to_file_id`].
    name_to_file_id: OnceLock<HashMap<String, u32>>,
}

impl SolxBuildModel {
    /// Creates a new instance from the provided compiler input and output.
    pub fn new(
        compiler_input: CompilerInput,
        compiler_output: &CompilerOutput<SolxBytecode>,
    ) -> anyhow::Result<Self> {
        let ast_spans = collect_ast_spans(compiler_output.sources.values());

        let CompiledContractsAndFiles {
            contract_id_to_contract,
            file_id_to_source_file,
        } = collect_compiled_contracts_and_files(compiler_input, compiler_output)?;

        Ok(Self {
            ast_spans,
            contract_id_to_contract,
            file_id_to_source_file: Arc::new(file_id_to_source_file),
            name_to_file_id: OnceLock::new(),
        })
    }

    #[cfg(test)]
    pub(crate) fn with_sources(
        file_id_to_source_file: HashMap<u32, Arc<RwLock<SourceFile>>>,
    ) -> Self {
        Self {
            ast_spans: HashMap::new(),
            contract_id_to_contract: IndexMap::new(),
            file_id_to_source_file: Arc::new(file_id_to_source_file),
            name_to_file_id: OnceLock::new(),
        }
    }

    pub fn file_id_by_name(&self, source_name: &str) -> Option<u32> {
        self.name_to_file_id().get(source_name).copied()
    }

    /// Reverse-index of `file_id_to_source_file` keyed by source name.
    /// Lazily populated on first call, reused thereafter.
    pub fn name_to_file_id(&self) -> &HashMap<String, u32> {
        self.name_to_file_id.get_or_init(|| {
            self.file_id_to_source_file
                .iter()
                .map(|(id, file)| (file.read().source_name.clone(), *id))
                .collect()
        })
    }

    /// Smallest (leafmost) AST `(offset, length)` span containing `offset`.
    /// Returns `None` if no span in `ast_spans[file_id]` covers `offset`.
    pub fn smallest_enclosing_span(&self, file_id: u32, offset: u32) -> Option<(u32, u32)> {
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

impl BuildModel for SolxBuildModel {
    type Artifact = SolxBytecode;

    fn contracts(&self) -> impl Iterator<Item = &Arc<RwLock<Contract>>> {
        self.contract_id_to_contract.values()
    }

    fn decode_instructions(
        &self,
        artifact: &Self::Artifact,
        normalized_code: &[u8],
        is_deployment: bool,
    ) -> anyhow::Result<Vec<super::Instruction>> {
        dwarf::decode_instructions(normalized_code, &artifact.debug_info, self, is_deployment)
            .map_err(Into::into)
    }

    fn source_model_by_file_id(&self, file_id: u32) -> Option<&Arc<RwLock<SourceFile>>> {
        self.file_id_to_source_file.get(&file_id)
    }
}
