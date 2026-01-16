//! Stack trace entries for Solidity errors.

use std::collections::HashMap;

use edr_chain_spec::HaltReasonTrait;
use edr_primitives::{Address, Bytes, U256};
use revm_inspectors::tracing::CallTraceArena;

use crate::{
    build_model::ContractFunctionType,
    contract_decoder::{ContractDecoderError, NestedTraceDecoder},
    nested_trace::{CallTraceArenaConversionError, NestedTrace},
    solidity_tracer::{self, SolidityTracerError},
};

pub(crate) const FALLBACK_FUNCTION_NAME: &str = "<fallback>";
pub(crate) const RECEIVE_FUNCTION_NAME: &str = "<receive>";
pub(crate) const CONSTRUCTOR_FUNCTION_NAME: &str = "constructor";
#[allow(unused)]
pub(crate) const UNKNOWN_FUNCTION_NAME: &str = "<unknown>";
#[allow(unused)]
pub(crate) const PRECOMPILE_FUNCTION_NAME: &str = "<precompile>";
/// Name used when we couldn't recognize the function.
pub const UNRECOGNIZED_FUNCTION_NAME: &str = "<unrecognized-selector>";
/// Name used when we couldn't recognize the contract.
pub const UNRECOGNIZED_CONTRACT_NAME: &str = "<UnrecognizedContract>";

/// A Solidity source reference.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SourceReference {
    /// The name of the source file.
    pub source_name: String,
    /// The content of the source file.
    pub source_content: String,
    /// The name of the contract.
    pub contract: Option<String>,
    /// The name of the function.
    pub function: Option<String>,
    /// The line number.
    pub line: u32,
    /// The character range on the line.
    pub range: (u32, u32),
}

// The names are self-explanatory.
#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub enum StackTraceEntry {
    CallstackEntry {
        source_reference: SourceReference,
        function_type: ContractFunctionType,
    },
    UnrecognizedCreateCallstackEntry,
    UnrecognizedContractCallstackEntry {
        address: Address,
    },
    PrecompileError {
        precompile: u32,
    },
    RevertError {
        return_data: Bytes,
        source_reference: SourceReference,
        is_invalid_opcode_error: bool,
    },
    PanicError {
        error_code: U256,
        source_reference: Option<SourceReference>,
    },
    CheatCodeError {
        message: String,
        source_reference: SourceReference,
    },
    CustomError {
        message: String,
        source_reference: SourceReference,
    },
    FunctionNotPayableError {
        value: U256,
        source_reference: SourceReference,
    },
    InvalidParamsError {
        source_reference: SourceReference,
    },
    FallbackNotPayableError {
        value: U256,
        source_reference: SourceReference,
    },
    FallbackNotPayableAndNoReceiveError {
        value: U256,
        source_reference: SourceReference,
    },
    // TODO: Should trying to call a private/internal be a special case of this?
    UnrecognizedFunctionWithoutFallbackError {
        source_reference: SourceReference,
    },
    MissingFallbackOrReceiveError {
        source_reference: SourceReference,
    },
    ReturndataSizeError {
        source_reference: SourceReference,
    },
    NoncontractAccountCalledError {
        source_reference: SourceReference,
    },
    CallFailedError {
        source_reference: SourceReference,
    },
    DirectLibraryCallError {
        source_reference: SourceReference,
    },
    UnrecognizedCreateError {
        return_data: Bytes,
        is_invalid_opcode_error: bool,
    },
    UnrecognizedContractError {
        address: Address,
        return_data: Bytes,
        is_invalid_opcode_error: bool,
    },
    OtherExecutionError {
        source_reference: Option<SourceReference>,
    },
    // This is a special case to handle a regression introduced in solc 0.6.3
    // For more info: https://github.com/ethereum/solidity/issues/9006
    UnmappedSolc0_6_3RevertError {
        source_reference: Option<SourceReference>,
    },
    ContractTooLargeError {
        source_reference: Option<SourceReference>,
    },
    InternalFunctionCallstackEntry {
        pc: u32,
        source_reference: SourceReference,
    },
    ContractCallRunOutOfGasError {
        source_reference: Option<SourceReference>,
    },
}

impl StackTraceEntry {
    /// Get the source reference of the stack trace entry if any.
    pub fn source_reference(&self) -> Option<&SourceReference> {
        match self {
            StackTraceEntry::CallstackEntry {
                source_reference, ..
            }
            | StackTraceEntry::RevertError {
                source_reference, ..
            }
            | StackTraceEntry::CheatCodeError {
                source_reference, ..
            }
            | StackTraceEntry::CustomError {
                source_reference, ..
            }
            | StackTraceEntry::FunctionNotPayableError {
                source_reference, ..
            }
            | StackTraceEntry::InvalidParamsError {
                source_reference, ..
            }
            | StackTraceEntry::FallbackNotPayableError {
                source_reference, ..
            }
            | StackTraceEntry::MissingFallbackOrReceiveError {
                source_reference, ..
            }
            | StackTraceEntry::ReturndataSizeError {
                source_reference, ..
            }
            | StackTraceEntry::NoncontractAccountCalledError {
                source_reference, ..
            }
            | StackTraceEntry::CallFailedError {
                source_reference, ..
            }
            | StackTraceEntry::DirectLibraryCallError {
                source_reference, ..
            }
            | StackTraceEntry::UnrecognizedFunctionWithoutFallbackError {
                source_reference, ..
            }
            | StackTraceEntry::InternalFunctionCallstackEntry {
                source_reference, ..
            }
            | StackTraceEntry::FallbackNotPayableAndNoReceiveError {
                source_reference, ..
            } => Some(source_reference),
            StackTraceEntry::PanicError {
                source_reference, ..
            }
            | StackTraceEntry::OtherExecutionError {
                source_reference, ..
            }
            | StackTraceEntry::UnmappedSolc0_6_3RevertError {
                source_reference, ..
            }
            | StackTraceEntry::ContractTooLargeError {
                source_reference, ..
            }
            | StackTraceEntry::ContractCallRunOutOfGasError {
                source_reference, ..
            } => source_reference.as_ref(),
            StackTraceEntry::PrecompileError { .. }
            | StackTraceEntry::UnrecognizedCreateError { .. }
            | StackTraceEntry::UnrecognizedCreateCallstackEntry
            | StackTraceEntry::UnrecognizedContractCallstackEntry { .. }
            | StackTraceEntry::UnrecognizedContractError { .. } => None,
        }
    }

    /// Whether the stack trace entry is an unrecognized contract call to the
    /// specified address.
    pub fn is_unrecognized_contract_call_error(&self, contract_address: &Address) -> bool {
        match self {
            StackTraceEntry::UnrecognizedContractCallstackEntry { address }
            | StackTraceEntry::UnrecognizedContractError { address, .. } => {
                address == contract_address
            }
            _ => false,
        }
    }
}

/// Stack trace creation error.
#[derive(Clone, Debug, thiserror::Error)]
pub enum StackTraceCreationError<HaltReasonT> {
    /// Error during contract decoding.
    #[error(transparent)]
    ContractDecoder(#[from] ContractDecoderError),
    /// Error during trace conversion
    #[error(transparent)]
    TraceConversion(#[from] CallTraceArenaConversionError),
    /// Error with the provided input trace.
    #[error(transparent)]
    Tracer(#[from] SolidityTracerError<HaltReasonT>),
}

impl<HaltReasonT> StackTraceCreationError<HaltReasonT> {
    /// Maps the type of the halt reason using the provided conversion function.
    pub fn map_halt_reason<
        ConversionFnT: Copy + Fn(HaltReasonT) -> NewHaltReasonT,
        NewHaltReasonT,
    >(
        self,
        conversion_fn: ConversionFnT,
    ) -> StackTraceCreationError<NewHaltReasonT> {
        match self {
            StackTraceCreationError::ContractDecoder(err) => {
                StackTraceCreationError::ContractDecoder(err)
            }
            StackTraceCreationError::TraceConversion(err) => {
                StackTraceCreationError::TraceConversion(err)
            }
            StackTraceCreationError::Tracer(err) => {
                StackTraceCreationError::Tracer(err.map_halt_reason(conversion_fn))
            }
        }
    }
}

/// Compute stack trace based on execution traces.
/// Assumes last trace is the error one. This is important for invariant tests
/// where there might be multiple errors traces. Returns `None` if `traces` is
/// empty.
pub fn get_stack_trace<
    'arena,
    HaltReasonT: HaltReasonTrait,
    NestedTraceDecoderT: NestedTraceDecoder<HaltReasonT>,
>(
    contract_decoder: &NestedTraceDecoderT,
    traces: impl IntoIterator<Item = &'arena CallTraceArena>,
) -> Result<Option<Vec<StackTraceEntry>>, StackTraceCreationError<HaltReasonT>> {
    let mut address_to_creation_code = HashMap::new();
    let mut address_to_runtime_code = HashMap::new();

    let last_trace = traces.into_iter().fold(None, |_, trace| {
        for node in trace.nodes() {
            let address = node.trace.address;
            if node.trace.kind.is_any_create() {
                address_to_creation_code.insert(address, &node.trace.data);
                address_to_runtime_code.insert(address, &node.trace.output);
            }
        }
        Some(trace)
    });

    if let Some(last_trace) = last_trace {
        let trace = NestedTrace::from_call_trace_arena(
            &address_to_creation_code,
            &address_to_runtime_code,
            last_trace,
        )?;
        let trace = contract_decoder.try_to_decode_nested_trace(trace)?;
        let stack_trace = solidity_tracer::get_stack_trace(trace)?;
        Ok(Some(stack_trace))
    } else {
        Ok(None)
    }
}

/// The possible outcomes from computing stack traces.
#[derive(Clone, Debug)]
pub enum StackTraceCreationResult<HaltReasonT> {
    /// The stack trace result
    Success(Vec<StackTraceEntry>),
    /// We couldn't generate stack traces, because an unexpected error occurred.
    Error(StackTraceCreationError<HaltReasonT>),
    /// We couldn't generate stack traces, because the heuristic failed.
    HeuristicFailed,
}

impl<HaltReasonT> StackTraceCreationResult<HaltReasonT> {
    /// Maps the type of the halt reason using the provided conversion function.
    pub fn map_halt_reason<
        ConversionFnT: Copy + Fn(HaltReasonT) -> NewHaltReasonT,
        NewHaltReasonT,
    >(
        self,
        conversion_fn: ConversionFnT,
    ) -> StackTraceCreationResult<NewHaltReasonT> {
        match self {
            StackTraceCreationResult::Success(stack_trace) => {
                StackTraceCreationResult::Success(stack_trace)
            }
            StackTraceCreationResult::Error(error) => {
                StackTraceCreationResult::Error(error.map_halt_reason(conversion_fn))
            }
            StackTraceCreationResult::HeuristicFailed => StackTraceCreationResult::HeuristicFailed,
        }
    }
}

impl<HaltReasonT: HaltReasonTrait>
    From<Result<Vec<StackTraceEntry>, StackTraceCreationError<HaltReasonT>>>
    for StackTraceCreationResult<HaltReasonT>
{
    fn from(value: Result<Vec<StackTraceEntry>, StackTraceCreationError<HaltReasonT>>) -> Self {
        match value {
            Ok(stack_trace) => {
                if stack_trace.is_empty() {
                    Self::HeuristicFailed
                } else {
                    Self::Success(stack_trace)
                }
            }
            Err(error) => Self::Error(error),
        }
    }
}
