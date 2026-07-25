//! Per-compiler stack-trace behaviour dispatched through the [`TraceStrategy`]
//! trait.

use std::sync::Arc;

use edr_primitives::Bytes;

use crate::{
    build_model::{
        ContractFunction, ContractFunctionType, ContractMetadata, ContractMetadataError,
        Instruction, SourceLocation,
    },
    solidity_stack_trace::{
        SourceReference, StackTraceEntry, CONSTRUCTOR_FUNCTION_NAME, FALLBACK_FUNCTION_NAME,
        RECEIVE_FUNCTION_NAME,
    },
};

/// Errors raised by [`TraceStrategy`] methods.
#[derive(Debug, thiserror::Error)]
pub enum TraceStrategyError {
    /// Source-reference resolution failed with no strategy fallback.
    #[error("Missing source reference")]
    MissingSourceReference,
    /// Underlying contract-metadata lookup error.
    #[error(transparent)]
    ContractMetadata(#[from] ContractMetadataError),
}

/// Trace-time inputs for [`TraceStrategy::panic_helper_source_reference`].
pub struct PanicHelperContext<'a> {
    /// Lazily yields the trace's EVM step PCs in execution order. A thunk
    /// because trace steps are generic over the halt-reason type, which
    /// can't cross the object-safe trait boundary.
    pub step_pcs: &'a dyn Fn() -> Vec<u32>,
    /// Calldata of the current Call frame; `None` for Create frames.
    pub calldata: Option<&'a Bytes>,
}

/// Compiler-specific stack-trace policy used by the error inferrer.
pub trait TraceStrategy: std::fmt::Debug + Send + Sync + 'static {
    /// Minimum `idx` at which same-location consecutive frames are treated
    /// as recursion in `filter_redundant_frames`. The filter runs
    /// per-message, so frame 0 can be a recursive call site (solx) rather
    /// than an entry point (solc).
    fn recursion_start_idx(&self) -> usize;

    /// Whether `step_location` counts as "execution is still at
    /// `reference_location`" when deciding that a statement was the last
    /// user code to run (`is_last_location`-style checks).
    fn locations_equivalent(
        &self,
        step_location: &SourceLocation,
        reference_location: &SourceLocation,
    ) -> bool;

    /// Failing function to attribute a revert to when the reverting
    /// instruction's location has no containing function (solx attributes
    /// shared helpers to the contract declaration). `None` leaves the
    /// generic fallback heuristics in charge.
    fn declaration_attributed_failing_function(
        &self,
        contract_meta: &ContractMetadata,
        calldata: Option<&Bytes>,
    ) -> Option<Arc<ContractFunction>>;

    /// Fallback frame when source-reference resolution returned `None` but
    /// the enclosing function is known.
    fn unresolved_callstack_entry(
        &self,
        contract_name: &str,
        inst_location: &SourceLocation,
    ) -> Result<StackTraceEntry, TraceStrategyError>;

    /// Extra frames inserted before the final revert / panic / custom frame.
    /// `bottom_source_reference` is that final frame's already-resolved
    /// source reference — the dedup anchor, so an intermediate frame naming
    /// the same function as the rendered bottom frame is dropped.
    fn intermediate_frames(
        &self,
        contract_meta: &ContractMetadata,
        last_instruction: &Instruction,
        bottom_source_reference: &SourceReference,
    ) -> Result<Vec<StackTraceEntry>, TraceStrategyError>;

    /// Source anchor for a revert that happened inside a known function;
    /// `inst_location` is `None` when the reverting instruction is unmapped
    /// (solx shared bare-revert helpers). `step_pcs` lazily yields the
    /// trace's EVM step PCs in execution order, for strategies that need to
    /// walk back from the reverting instruction.
    fn revert_source_reference(
        &self,
        contract_meta: &ContractMetadata,
        inst_location: Option<&SourceLocation>,
        failing_function: &ContractFunction,
        step_pcs: &dyn Fn() -> Vec<u32>,
    ) -> Result<SourceReference, TraceStrategyError>;

    /// Fallback source reference for a panic-helper PC when the primary
    /// resolution paths both returned `None`.
    fn panic_helper_source_reference(
        &self,
        primary_ref: Option<SourceReference>,
        contract_meta: &ContractMetadata,
        context: PanicHelperContext<'_>,
    ) -> Result<Option<SourceReference>, TraceStrategyError>;
}

/// Solc (sourceMap) trace-strategy impl.
#[derive(Debug)]
pub struct SolcTraceStrategy;

/// Global instance of [`SolcTraceStrategy`] used by the error inferrer.
pub static SOLC_TRACE_STRATEGY: SolcTraceStrategy = SolcTraceStrategy;

impl TraceStrategy for SolcTraceStrategy {
    fn recursion_start_idx(&self) -> usize {
        1
    }

    fn locations_equivalent(
        &self,
        step_location: &SourceLocation,
        reference_location: &SourceLocation,
    ) -> bool {
        step_location == reference_location
    }

    fn declaration_attributed_failing_function(
        &self,
        _contract_meta: &ContractMetadata,
        _calldata: Option<&Bytes>,
    ) -> Option<Arc<ContractFunction>> {
        None
    }

    fn unresolved_callstack_entry(
        &self,
        _contract_name: &str,
        _inst_location: &SourceLocation,
    ) -> Result<StackTraceEntry, TraceStrategyError> {
        Err(TraceStrategyError::MissingSourceReference)
    }

    fn intermediate_frames(
        &self,
        _contract_meta: &ContractMetadata,
        _last_instruction: &Instruction,
        _bottom_source_reference: &SourceReference,
    ) -> Result<Vec<StackTraceEntry>, TraceStrategyError> {
        Ok(Vec::new())
    }

    fn revert_source_reference(
        &self,
        contract_meta: &ContractMetadata,
        _inst_location: Option<&SourceLocation>,
        failing_function: &ContractFunction,
        _step_pcs: &dyn Fn() -> Vec<u32>,
    ) -> Result<SourceReference, TraceStrategyError> {
        function_start_source_reference(contract_meta, failing_function)
    }

    fn panic_helper_source_reference(
        &self,
        primary_ref: Option<SourceReference>,
        _contract_meta: &ContractMetadata,
        _context: PanicHelperContext<'_>,
    ) -> Result<Option<SourceReference>, TraceStrategyError> {
        Ok(primary_ref)
    }
}

/// Global instance of [`SolxTraceStrategy`] used by the error inferrer.
pub static SOLX_TRACE_STRATEGY: SolxTraceStrategy = SolxTraceStrategy;

/// Solx (DWARF) trace-strategy impl.
#[derive(Debug)]
pub struct SolxTraceStrategy;

impl TraceStrategy for SolxTraceStrategy {
    fn recursion_start_idx(&self) -> usize {
        0
    }

    fn locations_equivalent(
        &self,
        step_location: &SourceLocation,
        reference_location: &SourceLocation,
    ) -> bool {
        // solx attributes compiler-generated helper code (calldata decoding,
        // revert builders) to the enclosing function or contract
        // *declaration*, whose range contains the statement — where solc
        // leaves such code unmapped. Treat declaration-level padding as
        // "still at the statement".
        step_location == reference_location || step_location.contains(reference_location)
    }

    fn declaration_attributed_failing_function(
        &self,
        contract_meta: &ContractMetadata,
        calldata: Option<&Bytes>,
    ) -> Option<Arc<ContractFunction>> {
        let selector = calldata?.get(..4)?;
        let contract = contract_meta.contract.read();
        contract.get_function_from_selector(selector).cloned()
    }

    fn unresolved_callstack_entry(
        &self,
        contract_name: &str,
        inst_location: &SourceLocation,
    ) -> Result<StackTraceEntry, TraceStrategyError> {
        let file = inst_location.file()?;
        let file = file.read();

        Ok(StackTraceEntry::CallstackEntry {
            source_reference: SourceReference {
                function: None,
                contract: Some(contract_name.to_string()),
                source_name: file.source_name.clone(),
                source_content: file.content.clone(),
                line: inst_location.get_starting_line_number()?,
                range: (
                    inst_location.offset,
                    inst_location.offset + inst_location.length,
                ),
            },
            function_type: ContractFunctionType::Function,
        })
    }

    fn intermediate_frames(
        &self,
        contract_meta: &ContractMetadata,
        last_instruction: &Instruction,
        bottom_source_reference: &SourceReference,
    ) -> Result<Vec<StackTraceEntry>, TraceStrategyError> {
        // Dedup against the frame actually rendered below these, not the
        // raw instruction location: under line-0 emission (solx ≥0.1.6) the
        // instruction resolves to the flattened-into function's declaration
        // while the bottom frame's revert walk-back may land in a modifier —
        // the function frame between them is real, not a duplicate.
        let mut prev_function_name = bottom_source_reference.function.clone();
        let mut kept_innermost_first: Vec<SourceReference> = Vec::new();
        for call_site in &last_instruction.inline_call_sites {
            let Some(call_site_ref) =
                source_location_to_source_reference(contract_meta, Some(call_site))?
            else {
                continue;
            };
            if call_site_ref.function == prev_function_name {
                continue;
            }
            prev_function_name = call_site_ref.function.clone();
            kept_innermost_first.push(call_site_ref);
        }

        let mut frames: Vec<StackTraceEntry> = Vec::with_capacity(kept_innermost_first.len());
        for source_reference in kept_innermost_first.iter().rev().cloned() {
            frames.push(StackTraceEntry::CallstackEntry {
                source_reference,
                function_type: ContractFunctionType::Function,
            });
        }
        Ok(frames)
    }

    fn revert_source_reference(
        &self,
        contract_meta: &ContractMetadata,
        inst_location: Option<&SourceLocation>,
        failing_function: &ContractFunction,
        step_pcs: &dyn Fn() -> Vec<u32>,
    ) -> Result<SourceReference, TraceStrategyError> {
        // solx attributes shared revert helpers to the *declaration line* of
        // the function they were flattened into (e.g. a modifier's `require`
        // reverting at the modified function's signature line) or of the
        // enclosing contract, or leaves them unmapped entirely (bare
        // `revert()`). In all three cases, walk the executed steps backwards
        // to the statement that actually led here — the code preceding the
        // revert keeps its own line. Only statements of the failing function
        // itself or of a modifier (flattened into its frame, possibly from
        // another file) qualify; a statement of any other function marks the
        // end of the flattened frame, and the declaration-line reference is
        // kept.
        let needs_walk_back = match inst_location {
            Some(location) => is_declaration_attributed(contract_meta, location, failing_function)?,
            None => true,
        };
        if needs_walk_back {
            for pc in step_pcs().iter().rev() {
                let prev_inst = contract_meta.get_instruction(*pc)?;
                let Some(prev_location) = &prev_inst.location else {
                    continue;
                };
                if is_declaration_attributed(contract_meta, prev_location, failing_function)? {
                    continue;
                }
                let Some(containing_function) = prev_location.get_containing_function()? else {
                    continue;
                };
                if !failing_function.location.contains(prev_location)
                    && containing_function.r#type != ContractFunctionType::Modifier
                {
                    break;
                }
                if let Some(source_reference) =
                    source_location_to_source_reference(contract_meta, Some(prev_location))?
                {
                    return Ok(source_reference);
                }
            }
        }

        // Unmapped instructions and declaration-level padding can't be
        // turned into a source reference themselves; anchor at the failing
        // function's start instead.
        if let Some(source_reference) =
            source_location_to_source_reference(contract_meta, inst_location)?
        {
            return Ok(source_reference);
        }
        function_start_source_reference(contract_meta, failing_function)
    }

    fn panic_helper_source_reference(
        &self,
        primary_ref: Option<SourceReference>,
        contract_meta: &ContractMetadata,
        context: PanicHelperContext<'_>,
    ) -> Result<Option<SourceReference>, TraceStrategyError> {
        if let Some(r) = primary_ref {
            return Ok(Some(r));
        }
        for pc in (context.step_pcs)().iter().rev() {
            let prev_inst = contract_meta.get_instruction(*pc)?;
            let Some(loc) = &prev_inst.location else {
                continue;
            };
            if let Some(sref) = source_location_to_source_reference(contract_meta, Some(loc))? {
                return Ok(Some(sref));
            }
        }
        // Fall back to the start of the calldata-selector function.
        let Some(selector) = context.calldata.and_then(|calldata| calldata.get(..4)) else {
            return Ok(None);
        };
        let contract = contract_meta.contract.read();
        let Some(called_function) = contract.get_function_from_selector(selector) else {
            return Ok(None);
        };
        Ok(function_start_source_reference(contract_meta, called_function).ok())
    }
}

/// Non-halt-reason-generic source-location resolver used by
/// [`TraceStrategy`] impls and re-used from the error inferrer.
/// Whether `location` is solx declaration-level padding relative to
/// `failing_function`: on the function's or the enclosing contract's
/// declaration line, in the same file. Bare line numbers don't identify a
/// location across files, so file identity rides on
/// [`SourceLocation::contains`].
fn is_declaration_attributed(
    contract_meta: &ContractMetadata,
    location: &SourceLocation,
    failing_function: &ContractFunction,
) -> Result<bool, TraceStrategyError> {
    let line = location.get_starting_line_number()?;

    let function_location = &failing_function.location;
    if (function_location.contains(location) || location.contains(function_location))
        && line == function_location.get_starting_line_number()?
    {
        return Ok(true);
    }

    let contract = contract_meta.contract.read();
    let contract_location = &contract.location;
    if (contract_location.contains(location) || location.contains(contract_location))
        && line == contract_location.get_starting_line_number()?
    {
        return Ok(true);
    }

    Ok(false)
}

pub(crate) fn source_location_to_source_reference(
    contract_meta: &ContractMetadata,
    location: Option<&SourceLocation>,
) -> Result<Option<SourceReference>, TraceStrategyError> {
    let Some(location) = location else {
        return Ok(None);
    };
    let Some(func) = location.get_containing_function()? else {
        return Ok(None);
    };

    let func_name = match func.r#type {
        ContractFunctionType::Constructor => CONSTRUCTOR_FUNCTION_NAME.to_string(),
        ContractFunctionType::Fallback => FALLBACK_FUNCTION_NAME.to_string(),
        ContractFunctionType::Receive => RECEIVE_FUNCTION_NAME.to_string(),
        _ => func.name.clone(),
    };

    let func_location_file = func.location.file()?;
    let func_location_file = func_location_file.read();

    Ok(Some(SourceReference {
        function: Some(func_name),
        contract: if func.r#type == ContractFunctionType::FreeFunction {
            None
        } else {
            Some(contract_meta.contract.read().name.clone())
        },
        source_name: func_location_file.source_name.clone(),
        source_content: func_location_file.content.clone(),
        line: location.get_starting_line_number()?,
        range: (location.offset, location.offset + location.length),
    }))
}

/// Source reference anchored at a function's declaration. Non-halt-reason
/// generic counterpart of the error inferrer's function-start resolution,
/// shared by [`TraceStrategy`] impls.
pub(crate) fn function_start_source_reference(
    contract_meta: &ContractMetadata,
    func: &ContractFunction,
) -> Result<SourceReference, TraceStrategyError> {
    let contract = contract_meta.contract.read();

    let file = func.location.file()?;
    let file = file.read();

    let location = &func.location;

    Ok(SourceReference {
        source_name: file.source_name.clone(),
        source_content: file.content.clone(),
        contract: Some(contract.name.clone()),
        function: Some(func.name.clone()),
        line: location.get_starting_line_number()?,
        range: (location.offset, location.offset + location.length),
    })
}
