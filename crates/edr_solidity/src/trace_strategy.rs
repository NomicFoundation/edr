//! Per-compiler stack-trace behaviour dispatched through the [`TraceStrategy`]
//! trait.

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

    /// Fallback frame when source-reference resolution returned `None` but
    /// the enclosing function is known.
    fn unresolved_callstack_entry(
        &self,
        contract_name: &str,
        inst_location: &SourceLocation,
    ) -> Result<StackTraceEntry, TraceStrategyError>;

    /// Extra frames inserted before the final revert / panic / custom frame.
    fn intermediate_frames(
        &self,
        contract_meta: &ContractMetadata,
        last_instruction: &Instruction,
        failing_function: &ContractFunction,
    ) -> Result<Vec<StackTraceEntry>, TraceStrategyError>;

    /// Source anchor for a revert that happened at a specific instruction
    /// inside a known function. `step_pcs` lazily yields the trace's EVM
    /// step PCs in execution order, for strategies that need to walk back
    /// from the reverting instruction.
    fn revert_source_reference(
        &self,
        contract_meta: &ContractMetadata,
        inst_location: &SourceLocation,
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
        _failing_function: &ContractFunction,
    ) -> Result<Vec<StackTraceEntry>, TraceStrategyError> {
        Ok(Vec::new())
    }

    fn revert_source_reference(
        &self,
        contract_meta: &ContractMetadata,
        _inst_location: &SourceLocation,
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
        failing_function: &ContractFunction,
    ) -> Result<Vec<StackTraceEntry>, TraceStrategyError> {
        let bottom_func_name = match failing_function.r#type {
            ContractFunctionType::Constructor => Some(CONSTRUCTOR_FUNCTION_NAME.to_string()),
            ContractFunctionType::Fallback => Some(FALLBACK_FUNCTION_NAME.to_string()),
            ContractFunctionType::Receive => Some(RECEIVE_FUNCTION_NAME.to_string()),
            _ => Some(failing_function.name.clone()),
        };
        let mut prev_function_name = bottom_func_name;
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
        inst_location: &SourceLocation,
        failing_function: &ContractFunction,
        step_pcs: &dyn Fn() -> Vec<u32>,
    ) -> Result<SourceReference, TraceStrategyError> {
        // solx attributes shared revert helpers to the *declaration line* of
        // the function they were flattened into (e.g. a modifier's `require`
        // reverting at the modified function's signature line). When the
        // reverting instruction sits on the declaration line, walk the
        // executed steps backwards to the statement that actually led here —
        // the message-building code preceding the revert keeps its own line.
        let declaration_line = failing_function.location.get_starting_line_number()?;
        if inst_location.get_starting_line_number()? == declaration_line {
            for pc in step_pcs().iter().rev() {
                let prev_inst = contract_meta.get_instruction(*pc)?;
                let Some(prev_location) = &prev_inst.location else {
                    continue;
                };
                if prev_location.get_starting_line_number()? == declaration_line {
                    continue;
                }
                if let Some(source_reference) =
                    source_location_to_source_reference(contract_meta, Some(prev_location))?
                {
                    return Ok(source_reference);
                }
            }
        }

        source_location_to_source_reference(contract_meta, Some(inst_location))?
            .ok_or(TraceStrategyError::MissingSourceReference)
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
