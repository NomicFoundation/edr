//! Per-decode memo caches for the solx DWARF decoder.

use std::{collections::HashMap, sync::Arc};

use super::{SourceLookup, match_dwarf_to_build_model};
use crate::build_model::{ContractFunction, SourceLocation};

/// Per-decode memos; thousands of PCs resolve to few distinct keys.
#[derive(Default)]
pub(super) struct DecodeCaches {
    /// `file_id` → byte offset of each line start.
    line_starts: HashMap<u32, Vec<usize>>,
    /// `(file_id, offset)` → smallest enclosing AST span.
    enclosing_spans: HashMap<(u32, u32), Option<(u32, u32)>>,
    /// DWARF source path → matched `BuildModel` `file_id`.
    file_ids_by_name: HashMap<String, Option<u32>>,
    /// `(file_id, decl_line)` → function declared at that line.
    functions_by_decl_line: HashMap<(u32, u32), Option<Arc<ContractFunction>>>,
    /// `(file_id, offset)` → function whose span contains that offset.
    containing_functions: HashMap<(u32, u32), Option<Arc<ContractFunction>>>,
}

// The decode loop calls these per PC from another codegen unit; without
// `#[inline]` it runs ~0.4% more instructions.
impl DecodeCaches {
    #[inline]
    pub(super) fn resolve_file_id(
        &mut self,
        dwarf_name: &str,
        name_to_file_id: &HashMap<String, u32>,
    ) -> Option<u32> {
        if let Some(&cached) = self.file_ids_by_name.get(dwarf_name) {
            return cached;
        }
        let resolved = match_dwarf_to_build_model(dwarf_name, name_to_file_id);
        self.file_ids_by_name
            .insert(dwarf_name.to_owned(), resolved);
        resolved
    }

    #[inline]
    pub(super) fn line_start(
        &mut self,
        build_model: &impl SourceLookup,
        file_id: u32,
        line_idx: usize,
    ) -> Option<usize> {
        self.line_starts
            .entry(file_id)
            .or_insert_with(|| compute_line_starts(build_model, file_id))
            .get(line_idx)
            .copied()
    }

    #[inline]
    pub(super) fn enclosing_span(
        &mut self,
        build_model: &impl SourceLookup,
        file_id: u32,
        offset: u32,
    ) -> Option<(u32, u32)> {
        *self
            .enclosing_spans
            .entry((file_id, offset))
            .or_insert_with(|| build_model.smallest_enclosing_span(file_id, offset))
    }

    #[inline]
    pub(super) fn function_by_decl_line(
        &mut self,
        build_model: &impl SourceLookup,
        file_id: u32,
        decl_line: u32,
        dwarf_name: &str,
    ) -> Option<Arc<ContractFunction>> {
        self.functions_by_decl_line
            .entry((file_id, decl_line))
            .or_insert_with(|| {
                if let Some(file) = build_model.source_model_by_file_id(file_id) {
                    file.read().get_function_by_decl_line(decl_line).cloned()
                } else {
                    log::debug!(
                        "DWARF decoder: no source file for file_id {file_id} (dwarf_name={dwarf_name})"
                    );
                    None
                }
            })
            .clone()
    }

    #[inline]
    pub(super) fn containing_function(
        &mut self,
        build_model: &impl SourceLookup,
        file_id: u32,
        offset: u32,
        row_file: &str,
    ) -> Option<Arc<ContractFunction>> {
        self.containing_functions
            .entry((file_id, offset))
            .or_insert_with(|| {
                if let Some(source_file) = build_model.source_model_by_file_id(file_id) {
                    let probe = SourceLocation::new(source_file, offset, 0);
                    probe.get_containing_function().ok().flatten()
                } else {
                    log::debug!(
                        "DWARF decoder: no source file for file_id {file_id} (row.file={row_file})"
                    );
                    None
                }
            })
            .clone()
    }
}

fn compute_line_starts(build_model: &impl SourceLookup, file_id: u32) -> Vec<usize> {
    let Some(file) = build_model.source_model_by_file_id(file_id) else {
        return Vec::new();
    };
    let content = file.read().content.clone();
    let mut starts = vec![0usize];
    for (i, b) in content.bytes().enumerate() {
        if b == b'\n' {
            starts.push(i + 1);
        }
    }
    starts
}
