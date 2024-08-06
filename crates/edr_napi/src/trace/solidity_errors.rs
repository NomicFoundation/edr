use edr_eth::U256;
use edr_evm::hex;
use napi::{
    bindgen_prelude::{Either24, Undefined},
    Either,
};
use napi_derive::napi;

use super::{
    return_data::panic_error_code_to_message,
    solidity_stack_trace::{
        CallFailedErrorStackTraceEntry, ContractCallRunOutOfGasError,
        ContractTooLargeErrorStackTraceEntry, CustomErrorStackTraceEntry,
        DirectLibraryCallErrorStackTraceEntry, FallbackNotPayableAndNoReceiveErrorStackTraceEntry,
        FallbackNotPayableErrorStackTraceEntry, FunctionNotPayableErrorStackTraceEntry,
        InvalidParamsErrorStackTraceEntry, MissingFallbackOrReceiveErrorStackTraceEntry,
        NonContractAccountCalledErrorStackTraceEntry, OtherExecutionErrorStackTraceEntry,
        PanicErrorStackTraceEntry, PrecompileErrorStackTraceEntry,
        ReturndataSizeErrorStackTraceEntry, RevertErrorStackTraceEntry, SolidityStackTraceEntry,
        UnmappedSolc063RevertErrorStackTraceEntry, UnrecognizedContractErrorStackTraceEntry,
        UnrecognizedCreateErrorStackTraceEntry,
        UnrecognizedFunctionWithoutFallbackErrorStackTraceEntry,
    },
};

#[napi]
pub fn get_message_from_stack_trace_entry(
    entry: SolidityStackTraceEntry,
) -> napi::Result<Either<String, Undefined>> {
    match &entry {
        Either24::D(PrecompileErrorStackTraceEntry { precompile, .. }) => {
            let message = format!("Transaction reverted: call to precompile {precompile} failed");
            Ok(Either::A(message))
        }
        Either24::H(FunctionNotPayableErrorStackTraceEntry { value, .. }) => {
            let value = U256::checked_from_limbs_slice(&value.words).ok_or_else(|| {
                napi::Error::new(napi::Status::InvalidArg, "Expected value to fit uint256")
            })?;
            let message = format!("Transaction reverted: non-payable function was called with value {value}");
            Ok(Either::A(message))
        }
        Either24::I(InvalidParamsErrorStackTraceEntry { .. }) => {
            Ok(Either::A("Transaction reverted: function was called with incorrect parameters".to_string()))
        }
        Either24::J(FallbackNotPayableErrorStackTraceEntry { value, .. }) => {
            let value = U256::checked_from_limbs_slice(&value.words).ok_or_else(|| {
                napi::Error::new(napi::Status::InvalidArg, "Expected value to fit uint256")
            })?;
            let message = format!("Transaction reverted: fallback function is not payable and was called with value {value}");
            Ok(Either::A(message))
        }
        Either24::K(FallbackNotPayableAndNoReceiveErrorStackTraceEntry { value, .. }) => {
            let value = U256::checked_from_limbs_slice(&value.words).ok_or_else(|| {
                napi::Error::new(napi::Status::InvalidArg, "Expected value to fit uint256")
            })?;
            let message = format!(
                "Transaction reverted: there's no receive function, fallback function is not payable and was called with value {value}"
            );
            Ok(Either::A(message))
        }
        Either24::L(UnrecognizedFunctionWithoutFallbackErrorStackTraceEntry { .. }) => {
            Ok(Either::A("Transaction reverted: function selector was not recognized and there's no fallback function".to_string()))
        }
        Either24::M(MissingFallbackOrReceiveErrorStackTraceEntry { .. }) => {
            Ok(Either::A("Transaction reverted: function selector was not recognized and there's no fallback nor receive function".to_string()))
        }
        Either24::N(ReturndataSizeErrorStackTraceEntry { .. }) => {
            Ok(Either::A("Transaction reverted: function returned an unexpected amount of data".to_string()))
        }
        Either24::O(NonContractAccountCalledErrorStackTraceEntry { .. }) => {
            Ok(Either::A("Transaction reverted: function call to a non-contract account".to_string()))
        }
        Either24::P(CallFailedErrorStackTraceEntry { .. }) => {
            Ok(Either::A("Transaction reverted: function call failed to execute".to_string()))
        }
        Either24::Q(DirectLibraryCallErrorStackTraceEntry { .. }) => {
            Ok(Either::A("Transaction reverted: library was called directly".to_string()))
        }
        entry @ (Either24::R(UnrecognizedCreateErrorStackTraceEntry { .. }) | Either24::S(UnrecognizedContractErrorStackTraceEntry { .. })) => {
            let (entry_message, is_invalid_opcode_error) = match &entry {
                Either24::R(entry) => (&entry.message, entry.is_invalid_opcode_error),
                Either24::S(entry) => (&entry.message, entry.is_invalid_opcode_error),
                _ => unreachable!(),
            };
            if entry_message.is_error_return_data() {
                Ok(Either::A(format!(
                    "VM Exception while processing transaction: reverted with reason string '{}'",
                    entry_message.decode_error()?
                )))
            } else if entry_message.is_panic_return_data() {
                let message = panic_error_code_to_message(entry_message.decode_panic()?)?;
                Ok(Either::A(format!("VM Exception while processing transaction: {message}")))
            } else if !entry_message.is_empty() {
                Ok(Either::A(format!(
                    "VM Exception while processing transaction: reverted with an unrecognized custom error (return data: 0x{})",
                    hex::encode(&(**entry_message).value)
                )))
            } else if is_invalid_opcode_error {
                Ok(Either::A("VM Exception while processing transaction: invalid opcode".to_string()))
            } else {
                Ok(Either::A("Transaction reverted without a reason string".to_string()))
            }
        }
        Either24::E(entry @ RevertErrorStackTraceEntry { .. }) => {
            if entry.message.is_error_return_data() {
                Ok(Either::A(format!(
                    "VM Exception while processing transaction: reverted with reason string '{}'",
                    entry.message.decode_error()?
                )))
            } else if entry.is_invalid_opcode_error {
                Ok(Either::A("VM Exception while processing transaction: invalid opcode".to_string()))
            } else {
                Ok(Either::A("Transaction reverted without a reason string".to_string()))
            }
        }
        Either24::F(PanicErrorStackTraceEntry { error_code, .. }) => {
            let message = panic_error_code_to_message(error_code.clone())?;
            Ok(Either::A(format!("VM Exception while processing transaction: {message}")))
        }
        Either24::G(CustomErrorStackTraceEntry { message, .. }) => {
            Ok(Either::A(format!("VM Exception while processing transaction: {message}")))
        }
        Either24::T(OtherExecutionErrorStackTraceEntry { .. }) => {
            // TODO: What if there was returnData?
            Ok(Either::A("Transaction reverted and Hardhat couldn't infer the reason.".to_string()))
        }
        Either24::U(UnmappedSolc063RevertErrorStackTraceEntry { .. }) => {
            Ok(Either::A("Transaction reverted without a reason string and without a valid sourcemap provided by the compiler. Some line numbers may be off. We strongly recommend upgrading solc and always using revert reasons.".to_string()))
        }
        Either24::V(ContractTooLargeErrorStackTraceEntry { .. }) => {
            Ok(Either::A("Transaction reverted: trying to deploy a contract whose code is too large".to_string()))
        }
        Either24::X(ContractCallRunOutOfGasError { .. }) => {
            Ok(Either::A("Transaction reverted: contract call run out of gas and made the transaction revert".to_string()))
        }
        _ => Ok(Either::B(())),
    }
}
