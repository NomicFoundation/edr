//! Per-compiler stack-trace behaviour dispatched through the [`TraceStrategy`]
//! trait.

use std::sync::Arc;

use edr_defaults::SELECTOR_LEN;
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

/// Lazily yields the trace's EVM step PCs in execution order. A thunk
/// because trace steps are generic over the halt-reason type, which can't
/// cross the object-safe trait boundary.
pub type StepPcs<'a> = dyn Fn() -> Vec<u32> + 'a;

/// Trace-time inputs for [`TraceStrategy::panic_helper_source_reference`].
pub struct PanicHelperContext<'a> {
    /// EVM step PCs of the trace.
    pub step_pcs: &'a StepPcs<'a>,
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

    /// Whether a step at `step_location` counts as still executing the
    /// statement at `statement_location`, when deciding that a statement
    /// was the last user code to run (`is_last_location`-style checks).
    fn step_still_at_statement(
        &self,
        step_location: &SourceLocation,
        statement_location: &SourceLocation,
    ) -> bool;

    /// Failing function for a revert whose instruction location resolves to
    /// no containing function: solx recovers it from the calldata selector
    /// (shared revert helpers are unmapped or declaration-attributed); solc
    /// returns `None`, leaving the fallback heuristics in charge.
    fn failing_function_from_calldata(
        &self,
        contract_meta: &ContractMetadata,
        calldata: &Bytes,
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
        step_pcs: &StepPcs<'_>,
    ) -> Result<SourceReference, TraceStrategyError>;

    /// Fallback source reference for a panic-helper PC when the primary
    /// resolution paths both returned `None`.
    fn panic_helper_source_reference(
        &self,
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

    fn step_still_at_statement(
        &self,
        step_location: &SourceLocation,
        statement_location: &SourceLocation,
    ) -> bool {
        step_location == statement_location
    }

    fn failing_function_from_calldata(
        &self,
        _contract_meta: &ContractMetadata,
        _calldata: &Bytes,
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
        _step_pcs: &StepPcs<'_>,
    ) -> Result<SourceReference, TraceStrategyError> {
        function_start_source_reference(contract_meta, failing_function)
    }

    fn panic_helper_source_reference(
        &self,
        _contract_meta: &ContractMetadata,
        _context: PanicHelperContext<'_>,
    ) -> Result<Option<SourceReference>, TraceStrategyError> {
        Ok(None)
    }
}

/// Solx (DWARF) trace-strategy impl.
#[derive(Debug)]
pub struct SolxTraceStrategy;

/// Global instance of [`SolxTraceStrategy`] used by the error inferrer.
pub static SOLX_TRACE_STRATEGY: SolxTraceStrategy = SolxTraceStrategy;

impl TraceStrategy for SolxTraceStrategy {
    fn recursion_start_idx(&self) -> usize {
        0
    }

    fn step_still_at_statement(
        &self,
        step_location: &SourceLocation,
        statement_location: &SourceLocation,
    ) -> bool {
        // solx maps compiler-generated helper code to the enclosing
        // function or contract declaration, whose range contains the
        // statement; that padding still counts as "at the statement".
        step_location == statement_location || step_location.contains(statement_location)
    }

    fn failing_function_from_calldata(
        &self,
        contract_meta: &ContractMetadata,
        calldata: &Bytes,
    ) -> Option<Arc<ContractFunction>> {
        let selector = calldata.get(..SELECTOR_LEN)?;
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
        // Skip call sites repeating the function of the frame rendered
        // below them.
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

        Ok(kept_innermost_first
            .into_iter()
            .rev()
            .map(|source_reference| StackTraceEntry::CallstackEntry {
                source_reference,
                function_type: ContractFunctionType::Function,
            })
            .collect())
    }

    fn revert_source_reference(
        &self,
        contract_meta: &ContractMetadata,
        inst_location: Option<&SourceLocation>,
        failing_function: &ContractFunction,
        step_pcs: &StepPcs<'_>,
    ) -> Result<SourceReference, TraceStrategyError> {
        let needs_walk_back = match inst_location {
            Some(location) => is_declaration_attributed(contract_meta, location, failing_function)?,
            None => true,
        };
        if needs_walk_back
            && let Some(source_reference) =
                walk_back_to_last_statement(contract_meta, failing_function, step_pcs)?
        {
            return Ok(source_reference);
        }

        // Last resorts: the location's own reference (for declaration
        // padding, the declaration line), else the failing function's start
        // (unmapped instructions have no location to resolve).
        if let Some(source_reference) =
            source_location_to_source_reference(contract_meta, inst_location)?
        {
            return Ok(source_reference);
        }
        function_start_source_reference(contract_meta, failing_function)
    }

    fn panic_helper_source_reference(
        &self,
        contract_meta: &ContractMetadata,
        context: PanicHelperContext<'_>,
    ) -> Result<Option<SourceReference>, TraceStrategyError> {
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
        let Some(called_function) = context
            .calldata
            .and_then(|calldata| self.failing_function_from_calldata(contract_meta, calldata))
        else {
            return Ok(None);
        };
        function_start_source_reference(contract_meta, &called_function).map(Some)
    }
}

/// Walks the executed steps backwards to the last statement-level location
/// of `failing_function` (or of any modifier executed in this frame) — a
/// shared revert helper carries no statement line of its own, but the code
/// just before it keeps one. A statement of any other non-modifier function
/// ends the walk. Failure-path only: runs once per inferred revert and
/// stops at the first statement it finds.
fn walk_back_to_last_statement(
    contract_meta: &ContractMetadata,
    failing_function: &ContractFunction,
    step_pcs: &StepPcs<'_>,
) -> Result<Option<SourceReference>, TraceStrategyError> {
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
            return Ok(Some(source_reference));
        }
    }
    Ok(None)
}

/// Whether `location` is declaration-level padding for `failing_function`:
/// its own or the enclosing contract's declaration line, same file — line
/// numbers alone don't identify a location, hence the containment checks.
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

/// Non-halt-reason-generic source-location resolver used by
/// [`TraceStrategy`] impls and re-used from the error inferrer.
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
        // Raw AST name (empty for constructor/fallback/receive): the
        // hardhat-tests corpus pins this shape for function-start frames,
        // so do not map the names like source_location_to_source_reference.
        function: Some(func.name.clone()),
        line: location.get_starting_line_number()?,
        range: (location.offset, location.offset + location.length),
    })
}
