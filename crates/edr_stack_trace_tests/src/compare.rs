//! Compares an inferred stack trace with a case's expected entries.

use alloy_dyn_abi::{DynSolType, DynSolValue};
use anyhow::{bail, ensure};
use edr_primitives::{Bytes, U256};
use edr_solidity::solidity_stack_trace::{SourceReference, StackTraceEntry};

use crate::{corpus::ExpectedEntry, execution::parse_u256, CompilerConfig};

/// `Error(string)`
const ERROR_SELECTOR: [u8; 4] = [0x08, 0xc3, 0x79, 0xa0];

pub(crate) fn stack_traces(
    actual: &[StackTraceEntry],
    expected: &[ExpectedEntry],
    config: &CompilerConfig,
) -> anyhow::Result<()> {
    // via-IR output doesn't keep enough mapping for frames.
    let actual: Vec<&StackTraceEntry> = actual
        .iter()
        .filter(|entry| {
            !(config.is_via_ir() && matches!(entry, StackTraceEntry::CallstackEntry { .. }))
        })
        .collect();
    let expected: Vec<&ExpectedEntry> = expected
        .iter()
        .filter(|entry| !(config.is_via_ir() && entry.r#type == "CALLSTACK_ENTRY"))
        .collect();

    ensure!(
        actual.len() == expected.len(),
        "expected a trace of length {} but got {}:\n{}",
        expected.len(),
        actual.len(),
        describe(&actual)
    );

    for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
        entry(actual, expected, config)
            .map_err(|error| error.context(format!("entry {index}:\n{}", describe(&[actual]))))?;
    }
    Ok(())
}

fn entry(
    actual: &StackTraceEntry,
    expected: &ExpectedEntry,
    config: &CompilerConfig,
) -> anyhow::Result<()> {
    let actual_type = type_name(actual);
    if config.is_via_ir() && matches!(actual_type, "REVERT_ERROR" | "OTHER_EXECUTION_ERROR") {
        // via-IR merges revert paths; any revert is accepted.
        return Ok(());
    }
    ensure!(
        actual_type == expected.r#type,
        "type {actual_type}, expected {}",
        expected.r#type
    );

    let message = message(actual);
    let expected_message = expected.message.as_deref().unwrap_or_default();
    ensure!(
        message == expected_message,
        "message {message:?}, expected {expected_message:?}"
    );

    let value = value(actual);
    match (&expected.value, value) {
        (Some(expected), Some(value)) => {
            let expected = parse_u256(expected)?;
            ensure!(value == expected, "value {value}, expected {expected}");
        }
        (Some(_), None) => bail!("value missing"),
        (None, Some(value)) => bail!("unexpected value {value}"),
        (None, None) => {}
    }

    let error_code = error_code(actual).map(|code| format!("{code:x}"));
    ensure!(
        error_code == expected.error_code,
        "errorCode {error_code:?}, expected {:?}",
        expected.error_code
    );

    match (source_reference(actual), &expected.source_reference) {
        (None, None) => {}
        (Some(actual), None) => bail!("unexpected source reference {}", location(actual)),
        // via-IR output can leave a frame unmapped.
        (None, Some(_)) if config.is_via_ir() => {}
        (None, Some(_)) => bail!("source reference missing"),
        (Some(actual), Some(expected)) => {
            ensure!(
                actual.contract == expected.contract
                    && actual.source_name == expected.file
                    && actual.function == expected.function,
                "source reference {}, expected {}:{} {:?}.{:?}",
                location(actual),
                expected.file,
                expected.line,
                expected.contract,
                expected.function
            );
            if config.compares_lines() {
                ensure!(
                    actual.line == expected.line,
                    "line {}, expected {}",
                    actual.line,
                    expected.line
                );
            }
        }
    }
    Ok(())
}

fn type_name(entry: &StackTraceEntry) -> &'static str {
    match entry {
        StackTraceEntry::CallstackEntry { .. } => "CALLSTACK_ENTRY",
        StackTraceEntry::UnrecognizedCreateCallstackEntry => "UNRECOGNIZED_CREATE_CALLSTACK_ENTRY",
        StackTraceEntry::UnrecognizedContractCallstackEntry { .. } => {
            "UNRECOGNIZED_CONTRACT_CALLSTACK_ENTRY"
        }
        StackTraceEntry::PrecompileError { .. } => "PRECOMPILE_ERROR",
        StackTraceEntry::RevertError { .. } => "REVERT_ERROR",
        StackTraceEntry::PanicError { .. } => "PANIC_ERROR",
        StackTraceEntry::CheatCodeError { .. } => "CHEATCODE_ERROR",
        StackTraceEntry::CustomError { .. } => "CUSTOM_ERROR",
        StackTraceEntry::FunctionNotPayableError { .. } => "FUNCTION_NOT_PAYABLE_ERROR",
        StackTraceEntry::InvalidParamsError { .. } => "INVALID_PARAMS_ERROR",
        StackTraceEntry::FallbackNotPayableError { .. } => "FALLBACK_NOT_PAYABLE_ERROR",
        StackTraceEntry::FallbackNotPayableAndNoReceiveError { .. } => {
            "FALLBACK_NOT_PAYABLE_AND_NO_RECEIVE_ERROR"
        }
        StackTraceEntry::UnrecognizedFunctionWithoutFallbackError { .. } => {
            "UNRECOGNIZED_FUNCTION_WITHOUT_FALLBACK_ERROR"
        }
        StackTraceEntry::MissingFallbackOrReceiveError { .. } => {
            "MISSING_FALLBACK_OR_RECEIVE_ERROR"
        }
        StackTraceEntry::ReturndataSizeError { .. } => "RETURNDATA_SIZE_ERROR",
        StackTraceEntry::NoncontractAccountCalledError { .. } => "NONCONTRACT_ACCOUNT_CALLED_ERROR",
        StackTraceEntry::CallFailedError { .. } => "CALL_FAILED_ERROR",
        StackTraceEntry::DirectLibraryCallError { .. } => "DIRECT_LIBRARY_CALL_ERROR",
        StackTraceEntry::UnrecognizedCreateError { .. } => "UNRECOGNIZED_CREATE_ERROR",
        StackTraceEntry::UnrecognizedContractError { .. } => "UNRECOGNIZED_CONTRACT_ERROR",
        StackTraceEntry::OtherExecutionError { .. } => "OTHER_EXECUTION_ERROR",
        StackTraceEntry::UnmappedSolc0_6_3RevertError { .. } => "UNMAPPED_SOLC_0_6_3_REVERT_ERROR",
        StackTraceEntry::ContractTooLargeError { .. } => "CONTRACT_TOO_LARGE_ERROR",
        StackTraceEntry::InternalFunctionCallstackEntry { .. } => {
            "INTERNAL_FUNCTION_CALLSTACK_ENTRY"
        }
        StackTraceEntry::ContractCallRunOutOfGasError { .. } => {
            "CONTRACT_CALL_RUN_OUT_OF_GAS_ERROR"
        }
    }
}

/// The parsed message, or the `Error(string)` reason in the return data.
fn message(entry: &StackTraceEntry) -> String {
    match entry {
        StackTraceEntry::CustomError { message, .. }
        | StackTraceEntry::CheatCodeError { message, .. } => message.clone(),
        StackTraceEntry::RevertError { return_data, .. }
        | StackTraceEntry::UnrecognizedCreateError { return_data, .. }
        | StackTraceEntry::UnrecognizedContractError { return_data, .. } => {
            error_reason(return_data).unwrap_or_default()
        }
        _ => String::new(),
    }
}

fn error_reason(return_data: &Bytes) -> Option<String> {
    let (selector, payload) = return_data.split_first_chunk::<4>()?;
    if *selector != ERROR_SELECTOR {
        return None;
    }
    match DynSolType::String.abi_decode(payload).ok()? {
        DynSolValue::String(reason) => Some(reason),
        _ => None,
    }
}

fn value(entry: &StackTraceEntry) -> Option<U256> {
    match entry {
        StackTraceEntry::FunctionNotPayableError { value, .. }
        | StackTraceEntry::FallbackNotPayableError { value, .. }
        | StackTraceEntry::FallbackNotPayableAndNoReceiveError { value, .. } => Some(*value),
        _ => None,
    }
}

fn error_code(entry: &StackTraceEntry) -> Option<U256> {
    match entry {
        StackTraceEntry::PanicError { error_code, .. } => Some(*error_code),
        _ => None,
    }
}

fn source_reference(entry: &StackTraceEntry) -> Option<&SourceReference> {
    match entry {
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
        | StackTraceEntry::InvalidParamsError { source_reference }
        | StackTraceEntry::FallbackNotPayableError {
            source_reference, ..
        }
        | StackTraceEntry::FallbackNotPayableAndNoReceiveError {
            source_reference, ..
        }
        | StackTraceEntry::UnrecognizedFunctionWithoutFallbackError { source_reference }
        | StackTraceEntry::MissingFallbackOrReceiveError { source_reference }
        | StackTraceEntry::ReturndataSizeError { source_reference }
        | StackTraceEntry::NoncontractAccountCalledError { source_reference }
        | StackTraceEntry::CallFailedError { source_reference }
        | StackTraceEntry::DirectLibraryCallError { source_reference }
        | StackTraceEntry::InternalFunctionCallstackEntry {
            source_reference, ..
        } => Some(source_reference),
        StackTraceEntry::PanicError {
            source_reference, ..
        }
        | StackTraceEntry::OtherExecutionError { source_reference }
        | StackTraceEntry::UnmappedSolc0_6_3RevertError { source_reference }
        | StackTraceEntry::ContractTooLargeError { source_reference }
        | StackTraceEntry::ContractCallRunOutOfGasError { source_reference } => {
            source_reference.as_ref()
        }
        StackTraceEntry::UnrecognizedCreateCallstackEntry
        | StackTraceEntry::UnrecognizedContractCallstackEntry { .. }
        | StackTraceEntry::PrecompileError { .. }
        | StackTraceEntry::UnrecognizedCreateError { .. }
        | StackTraceEntry::UnrecognizedContractError { .. } => None,
    }
}

fn location(source_reference: &SourceReference) -> String {
    format!(
        "{}:{} {:?}.{:?}",
        source_reference.source_name,
        source_reference.line,
        source_reference.contract,
        source_reference.function
    )
}

fn describe(entries: &[&StackTraceEntry]) -> String {
    entries
        .iter()
        .map(|entry| {
            let location = source_reference(entry).map_or_else(String::new, location);
            format!("  {} {location}", type_name(entry))
        })
        .collect::<Vec<_>>()
        .join("\n")
}
