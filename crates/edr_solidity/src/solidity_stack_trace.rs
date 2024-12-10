//! Stack trace entries for Solidity errors.

use edr_eth::{Address, Bytes, U256};

use crate::build_model::ContractFunctionType;

pub(crate) const FALLBACK_FUNCTION_NAME: &str = "<fallback>";
pub(crate) const RECEIVE_FUNCTION_NAME: &str = "<receive>";
pub(crate) const CONSTRUCTOR_FUNCTION_NAME: &str = "constructor";
pub(crate) const UNRECOGNIZED_FUNCTION_NAME: &str = "<unrecognized-selector>";
#[allow(unused)]
pub(crate) const UNKNOWN_FUNCTION_NAME: &str = "<unknown>";
#[allow(unused)]
pub(crate) const PRECOMPILE_FUNCTION_NAME: &str = "<precompile>";
pub(crate) const UNRECOGNIZED_CONTRACT_NAME: &str = "<UnrecognizedContract>";

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
}
