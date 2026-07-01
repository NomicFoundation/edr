//! Per-compiler stack-trace behaviour dispatched through the [`TraceStrategy`]
//! trait, accessed at call sites via a [`TraceContext`] façade.

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

/// Trace-time data pre-computed by the caller so [`TraceStrategy`] can stay
/// non-generic over the halt-reason type parameter.
pub struct PanicHelperContext<'a> {
    /// EVM-only step PCs in call order.
    pub step_pcs: &'a [u32],
    /// Function-start reference for the calldata-selector function, if any.
    pub call_selector_function_start_ref: Option<SourceReference>,
}

/// Compiler-specific stack-trace policy used by [`crate::error_inferrer`].
pub trait TraceStrategy: std::fmt::Debug + Send + Sync + 'static {
    /// Minimum `idx` at which same-location consecutive frames are treated
    /// as recursion in `filter_redundant_frames`.
    fn recursion_start_idx(&self) -> usize;

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
    /// inside a known function.
    fn revert_source_reference(
        &self,
        contract_meta: &ContractMetadata,
        inst_location: &SourceLocation,
        failing_function_start_ref: SourceReference,
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

/// Bundles `ContractMetadata` + [`TraceStrategy`] into one call-site handle.
#[derive(Clone, Copy)]
pub struct TraceContext<'a> {
    /// Contract metadata for the current frame.
    pub contract_meta: &'a ContractMetadata,
    /// Compiler-specific stack-trace strategy for that contract.
    pub strategy: &'a dyn TraceStrategy,
}

impl<'a> TraceContext<'a> {
    /// See [`TraceStrategy::recursion_start_idx`].
    pub fn recursion_start_idx(&self) -> usize {
        self.strategy.recursion_start_idx()
    }

    /// See [`TraceStrategy::unresolved_callstack_entry`].
    pub fn unresolved_callstack_entry(
        &self,
        contract_name: &str,
        inst_location: &SourceLocation,
    ) -> Result<StackTraceEntry, TraceStrategyError> {
        self.strategy
            .unresolved_callstack_entry(contract_name, inst_location)
    }

    /// See [`TraceStrategy::intermediate_frames`].
    pub fn intermediate_frames(
        &self,
        last_instruction: &Instruction,
        failing_function: &ContractFunction,
    ) -> Result<Vec<StackTraceEntry>, TraceStrategyError> {
        self.strategy
            .intermediate_frames(self.contract_meta, last_instruction, failing_function)
    }

    /// See [`TraceStrategy::revert_source_reference`].
    pub fn revert_source_reference(
        &self,
        inst_location: &SourceLocation,
        failing_function_start_ref: SourceReference,
    ) -> Result<SourceReference, TraceStrategyError> {
        self.strategy.revert_source_reference(
            self.contract_meta,
            inst_location,
            failing_function_start_ref,
        )
    }

    /// See [`TraceStrategy::panic_helper_source_reference`].
    pub fn panic_helper_source_reference(
        &self,
        primary_ref: Option<SourceReference>,
        context: PanicHelperContext<'_>,
    ) -> Result<Option<SourceReference>, TraceStrategyError> {
        self.strategy
            .panic_helper_source_reference(primary_ref, self.contract_meta, context)
    }
}

/// Solc (sourceMap) trace-strategy impl.
#[derive(Debug)]
pub struct SolcTraceStrategy;

impl TraceStrategy for SolcTraceStrategy {
    fn recursion_start_idx(&self) -> usize {
        1
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
        _contract_meta: &ContractMetadata,
        _inst_location: &SourceLocation,
        failing_function_start_ref: SourceReference,
    ) -> Result<SourceReference, TraceStrategyError> {
        Ok(failing_function_start_ref)
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

/// Solx (DWARF) trace-strategy impl.
#[derive(Debug)]
pub struct SolxTraceStrategy;

impl TraceStrategy for SolxTraceStrategy {
    fn recursion_start_idx(&self) -> usize {
        0
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
        _failing_function_start_ref: SourceReference,
    ) -> Result<SourceReference, TraceStrategyError> {
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
        for &pc in context.step_pcs.iter().rev() {
            let prev_inst = contract_meta.get_instruction(pc)?;
            let Some(loc) = &prev_inst.location else {
                continue;
            };
            if let Some(sref) = source_location_to_source_reference(contract_meta, Some(loc))? {
                return Ok(Some(sref));
            }
        }
        Ok(context.call_selector_function_start_ref)
    }
}

/// Non-halt-reason-generic source-location resolver used by
/// [`TraceStrategy`] impls and re-used from [`crate::error_inferrer`].
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
