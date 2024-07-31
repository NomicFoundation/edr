//! Naive rewrite of `hardhat-network/stack-traces/solidity-stack-traces.ts` from Hardhat.

use napi::bindgen_prelude::{
    BigInt, Either24, FromNapiValue, Object, ToNapiValue, Uint8Array, Undefined,
};
use napi_derive::napi;

use super::model::ContractFunctionType;

#[napi]
#[repr(u8)]
#[allow(non_camel_case_types)] // intentionally mimicks the original case in TS
#[allow(clippy::upper_case_acronyms)]
#[derive(PartialEq, PartialOrd, strum::FromRepr, strum::IntoStaticStr)]
pub enum StackTraceEntryType {
    CALLSTACK_ENTRY = 0,
    UNRECOGNIZED_CREATE_CALLSTACK_ENTRY,
    UNRECOGNIZED_CONTRACT_CALLSTACK_ENTRY,
    PRECOMPILE_ERROR,
    REVERT_ERROR,
    PANIC_ERROR,
    CUSTOM_ERROR,
    FUNCTION_NOT_PAYABLE_ERROR,
    INVALID_PARAMS_ERROR,
    FALLBACK_NOT_PAYABLE_ERROR,
    FALLBACK_NOT_PAYABLE_AND_NO_RECEIVE_ERROR,
    UNRECOGNIZED_FUNCTION_WITHOUT_FALLBACK_ERROR, // TODO: Should trying to call a private/internal be a special case of this?
    MISSING_FALLBACK_OR_RECEIVE_ERROR,
    RETURNDATA_SIZE_ERROR,
    NONCONTRACT_ACCOUNT_CALLED_ERROR,
    CALL_FAILED_ERROR,
    DIRECT_LIBRARY_CALL_ERROR,
    UNRECOGNIZED_CREATE_ERROR,
    UNRECOGNIZED_CONTRACT_ERROR,
    OTHER_EXECUTION_ERROR,
    // This is a special case to handle a regression introduced in solc 0.6.3
    // For more info: https://github.com/ethereum/solidity/issues/9006
    UNMAPPED_SOLC_0_6_3_REVERT_ERROR,
    CONTRACT_TOO_LARGE_ERROR,
    INTERNAL_FUNCTION_CALLSTACK_ENTRY,
    CONTRACT_CALL_RUN_OUT_OF_GAS_ERROR,
}

#[napi]
pub fn stack_trace_entry_type_to_string(val: StackTraceEntryType) -> &'static str {
    val.into()
}

#[napi]
pub const FALLBACK_FUNCTION_NAME: &str = "<fallback>";
#[napi]
pub const RECEIVE_FUNCTION_NAME: &str = "<receive>";
#[napi]
pub const CONSTRUCTOR_FUNCTION_NAME: &str = "constructor";
#[napi]
pub const UNRECOGNIZED_FUNCTION_NAME: &str = "<unrecognized-selector>";
#[napi]
pub const UNKNOWN_FUNCTION_NAME: &str = "<unknown>";
#[napi]
pub const PRECOMPILE_FUNCTION_NAME: &str = "<precompile>";
#[napi]
pub const UNRECOGNIZED_CONTRACT_NAME: &str = "<UnrecognizedContract>";

#[napi(object)]
pub struct SourceReference {
    pub source_name: String,
    pub source_content: String,
    pub contract: Option<String>,
    pub function: Option<String>,
    pub line: u32,
    // [number, number] tuple
    pub range: Vec<u32>,
}

/// A [`StackTraceEntryType`] constant that is convertible to/from a `napi_value`.
///
/// Since Rust does not allow constants directly as members, we use this wrapper
/// to allow the `StackTraceEntryType` to be used as a member of an interface
/// when defining the N-API bindings.
// NOTE: It's currently not possible to use an enum as const generic parameter,
// so we use the underlying `u8` repr used by the enum.
pub struct StackTraceEntryTypeConst<const ENTRY_TYPE: u8>;
impl<const ENTRY_TYPE: u8> FromNapiValue for StackTraceEntryTypeConst<ENTRY_TYPE> {
    unsafe fn from_napi_value(
        env: napi::sys::napi_env,
        napi_val: napi::sys::napi_value,
    ) -> napi::Result<Self> {
        let inner: u8 = FromNapiValue::from_napi_value(env, napi_val)?;

        if inner != ENTRY_TYPE {
            return Err(napi::Error::new(
                napi::Status::InvalidArg,
                format!("Expected StackTraceEntryType value: {ENTRY_TYPE}, got: {inner}"),
            ));
        }

        Ok(StackTraceEntryTypeConst)
    }
}
impl<const ENTRY_TYPE: u8> ToNapiValue for StackTraceEntryTypeConst<ENTRY_TYPE> {
    unsafe fn to_napi_value(
        env: napi::sys::napi_env,
        _val: Self,
    ) -> napi::Result<napi::sys::napi_value> {
        u8::to_napi_value(env, ENTRY_TYPE)
    }
}

#[napi(object)]
pub struct CallstackEntryStackTraceEntry {
    #[napi(js_name = "type", ts_type = "StackTraceEntryType.CALLSTACK_ENTRY")]
    pub type_: StackTraceEntryTypeConst<{ StackTraceEntryType::CALLSTACK_ENTRY as u8 }>,
    pub source_reference: SourceReference,
    pub function_type: ContractFunctionType,
}

#[napi(object)]
pub struct UnrecognizedCreateCallstackEntryStackTraceEntry {
    #[napi(
        js_name = "type",
        ts_type = "StackTraceEntryType.UNRECOGNIZED_CREATE_CALLSTACK_ENTRY"
    )]
    pub type_: StackTraceEntryTypeConst<
        { StackTraceEntryType::UNRECOGNIZED_CREATE_CALLSTACK_ENTRY as u8 },
    >,
    pub source_reference: Option<Undefined>,
}

#[napi(object)]
pub struct UnrecognizedContractCallstackEntryStackTraceEntry {
    #[napi(
        js_name = "type",
        ts_type = "StackTraceEntryType.UNRECOGNIZED_CONTRACT_CALLSTACK_ENTRY"
    )]
    pub type_: StackTraceEntryTypeConst<
        { StackTraceEntryType::UNRECOGNIZED_CONTRACT_CALLSTACK_ENTRY as u8 },
    >,
    pub address: Uint8Array,
    pub source_reference: Option<Undefined>,
}

#[napi(object)]
pub struct PrecompileErrorStackTraceEntry {
    #[napi(js_name = "type", ts_type = "StackTraceEntryType.PRECOMPILE_ERROR")]
    pub type_: StackTraceEntryTypeConst<{ StackTraceEntryType::PRECOMPILE_ERROR as u8 }>,
    pub precompile: u32,
    pub source_reference: Option<Undefined>,
}

#[napi(object)]
pub struct RevertErrorStackTraceEntry {
    #[napi(js_name = "type", ts_type = "StackTraceEntryType.REVERT_ERROR")]
    pub type_: StackTraceEntryTypeConst<{ StackTraceEntryType::REVERT_ERROR as u8 }>,
    #[napi(ts_type = "ReturnData")]
    pub message: Object,
    pub source_reference: SourceReference,
    pub is_invalid_opcode_error: bool,
}

#[napi(object)]
pub struct PanicErrorStackTraceEntry {
    #[napi(js_name = "type", ts_type = "StackTraceEntryType.PANIC_ERROR")]
    pub type_: StackTraceEntryTypeConst<{ StackTraceEntryType::PANIC_ERROR as u8 }>,
    pub error_code: BigInt,
    pub source_reference: Option<SourceReference>,
}

#[napi(object)]
pub struct CustomErrorStackTraceEntry {
    #[napi(js_name = "type", ts_type = "StackTraceEntryType.CUSTOM_ERROR")]
    pub type_: StackTraceEntryTypeConst<{ StackTraceEntryType::CUSTOM_ERROR as u8 }>,
    // unlike RevertErrorStackTraceEntry, this includes the message already parsed
    pub message: String,
    pub source_reference: SourceReference,
}

#[napi(object)]
pub struct UnmappedSolc063RevertErrorStackTraceEntry {
    #[napi(
        js_name = "type",
        ts_type = "StackTraceEntryType.UNMAPPED_SOLC_0_6_3_REVERT_ERROR"
    )]
    pub type_:
        StackTraceEntryTypeConst<{ StackTraceEntryType::UNMAPPED_SOLC_0_6_3_REVERT_ERROR as u8 }>,
    pub source_reference: Option<SourceReference>,
}

#[napi(object)]
pub struct FunctionNotPayableErrorStackTraceEntry {
    #[napi(
        js_name = "type",
        ts_type = "StackTraceEntryType.FUNCTION_NOT_PAYABLE_ERROR"
    )]
    pub type_: StackTraceEntryTypeConst<{ StackTraceEntryType::FUNCTION_NOT_PAYABLE_ERROR as u8 }>,
    pub value: BigInt,
    pub source_reference: SourceReference,
}

#[napi(object)]
pub struct InvalidParamsErrorStackTraceEntry {
    #[napi(js_name = "type", ts_type = "StackTraceEntryType.INVALID_PARAMS_ERROR")]
    pub type_: StackTraceEntryTypeConst<{ StackTraceEntryType::INVALID_PARAMS_ERROR as u8 }>,
    pub source_reference: SourceReference,
}

#[napi(object)]
pub struct FallbackNotPayableErrorStackTraceEntry {
    #[napi(
        js_name = "type",
        ts_type = "StackTraceEntryType.FALLBACK_NOT_PAYABLE_ERROR"
    )]
    pub type_: StackTraceEntryTypeConst<{ StackTraceEntryType::FALLBACK_NOT_PAYABLE_ERROR as u8 }>,
    pub value: BigInt,
    pub source_reference: SourceReference,
}

#[napi(object)]
pub struct FallbackNotPayableAndNoReceiveErrorStackTraceEntry {
    #[napi(
        js_name = "type",
        ts_type = "StackTraceEntryType.FALLBACK_NOT_PAYABLE_AND_NO_RECEIVE_ERROR"
    )]
    pub type_: StackTraceEntryTypeConst<
        { StackTraceEntryType::FALLBACK_NOT_PAYABLE_AND_NO_RECEIVE_ERROR as u8 },
    >,
    pub value: BigInt,
    pub source_reference: SourceReference,
}

#[napi(object)]
pub struct UnrecognizedFunctionWithoutFallbackErrorStackTraceEntry {
    #[napi(
        js_name = "type",
        ts_type = "StackTraceEntryType.UNRECOGNIZED_FUNCTION_WITHOUT_FALLBACK_ERROR"
    )]
    pub type_: StackTraceEntryTypeConst<
        { StackTraceEntryType::UNRECOGNIZED_FUNCTION_WITHOUT_FALLBACK_ERROR as u8 },
    >,
    pub source_reference: SourceReference,
}

#[napi(object)]
pub struct MissingFallbackOrReceiveErrorStackTraceEntry {
    #[napi(
        js_name = "type",
        ts_type = "StackTraceEntryType.MISSING_FALLBACK_OR_RECEIVE_ERROR"
    )]
    pub type_:
        StackTraceEntryTypeConst<{ StackTraceEntryType::MISSING_FALLBACK_OR_RECEIVE_ERROR as u8 }>,
    pub source_reference: SourceReference,
}

#[napi(object)]
pub struct ReturndataSizeErrorStackTraceEntry {
    #[napi(
        js_name = "type",
        ts_type = "StackTraceEntryType.RETURNDATA_SIZE_ERROR"
    )]
    pub type_: StackTraceEntryTypeConst<{ StackTraceEntryType::RETURNDATA_SIZE_ERROR as u8 }>,
    pub source_reference: SourceReference,
}

#[napi(object)]
pub struct NonContractAccountCalledErrorStackTraceEntry {
    #[napi(
        js_name = "type",
        ts_type = "StackTraceEntryType.NONCONTRACT_ACCOUNT_CALLED_ERROR"
    )]
    pub type_:
        StackTraceEntryTypeConst<{ StackTraceEntryType::NONCONTRACT_ACCOUNT_CALLED_ERROR as u8 }>,
    pub source_reference: SourceReference,
}

#[napi(object)]
pub struct CallFailedErrorStackTraceEntry {
    #[napi(js_name = "type", ts_type = "StackTraceEntryType.CALL_FAILED_ERROR")]
    pub type_: StackTraceEntryTypeConst<{ StackTraceEntryType::CALL_FAILED_ERROR as u8 }>,
    pub source_reference: SourceReference,
}
#[napi(object)]
pub struct DirectLibraryCallErrorStackTraceEntry {
    #[napi(
        js_name = "type",
        ts_type = "StackTraceEntryType.DIRECT_LIBRARY_CALL_ERROR"
    )]
    pub type_: StackTraceEntryTypeConst<{ StackTraceEntryType::DIRECT_LIBRARY_CALL_ERROR as u8 }>,
    pub source_reference: SourceReference,
}

#[napi(object)]
pub struct UnrecognizedCreateErrorStackTraceEntry {
    #[napi(
        js_name = "type",
        ts_type = "StackTraceEntryType.UNRECOGNIZED_CREATE_ERROR"
    )]
    pub type_: StackTraceEntryTypeConst<{ StackTraceEntryType::UNRECOGNIZED_CREATE_ERROR as u8 }>,
    #[napi(ts_type = "ReturnData")]
    pub message: Object,
    pub source_reference: Option<Undefined>,
    pub is_invalid_opcode_error: bool,
}

#[napi(object)]
pub struct UnrecognizedContractErrorStackTraceEntry {
    #[napi(
        js_name = "type",
        ts_type = "StackTraceEntryType.UNRECOGNIZED_CONTRACT_ERROR"
    )]
    pub type_: StackTraceEntryTypeConst<{ StackTraceEntryType::UNRECOGNIZED_CONTRACT_ERROR as u8 }>,
    pub address: Uint8Array,
    #[napi(ts_type = "ReturnData")]
    pub message: Object,
    pub source_reference: Option<Undefined>,
    pub is_invalid_opcode_error: bool,
}

#[napi(object)]
pub struct OtherExecutionErrorStackTraceEntry {
    #[napi(
        js_name = "type",
        ts_type = "StackTraceEntryType.OTHER_EXECUTION_ERROR"
    )]
    pub type_: StackTraceEntryTypeConst<{ StackTraceEntryType::OTHER_EXECUTION_ERROR as u8 }>,
    pub source_reference: Option<SourceReference>,
}

#[napi(object)]
pub struct ContractTooLargeErrorStackTraceEntry {
    #[napi(
        js_name = "type",
        ts_type = "StackTraceEntryType.CONTRACT_TOO_LARGE_ERROR"
    )]
    pub type_: StackTraceEntryTypeConst<{ StackTraceEntryType::CONTRACT_TOO_LARGE_ERROR as u8 }>,
    pub source_reference: Option<SourceReference>,
}

#[napi(object)]
pub struct InternalFunctionCallStackEntry {
    #[napi(
        js_name = "type",
        ts_type = "StackTraceEntryType.INTERNAL_FUNCTION_CALLSTACK_ENTRY"
    )]
    pub type_:
        StackTraceEntryTypeConst<{ StackTraceEntryType::INTERNAL_FUNCTION_CALLSTACK_ENTRY as u8 }>,
    pub pc: u32,
    pub source_reference: SourceReference,
}

#[napi(object)]
pub struct ContractCallRunOutOfGasError {
    #[napi(
        js_name = "type",
        ts_type = "StackTraceEntryType.CONTRACT_CALL_RUN_OUT_OF_GAS_ERROR"
    )]
    pub type_:
        StackTraceEntryTypeConst<{ StackTraceEntryType::CONTRACT_CALL_RUN_OUT_OF_GAS_ERROR as u8 }>,
    pub source_reference: Option<SourceReference>,
}

#[allow(dead_code)]
// NOTE: This ported directly from JS for completeness, however the type must be
// used verbatim in JS definitions because napi-rs does not store not allows to
// reuse the same type unless fully specified at definition site.
pub type SolidityStackTraceEntry = Either24<
    CallstackEntryStackTraceEntry,
    UnrecognizedCreateCallstackEntryStackTraceEntry,
    UnrecognizedContractCallstackEntryStackTraceEntry,
    PrecompileErrorStackTraceEntry,
    RevertErrorStackTraceEntry,
    PanicErrorStackTraceEntry,
    CustomErrorStackTraceEntry,
    FunctionNotPayableErrorStackTraceEntry,
    InvalidParamsErrorStackTraceEntry,
    FallbackNotPayableErrorStackTraceEntry,
    FallbackNotPayableAndNoReceiveErrorStackTraceEntry,
    UnrecognizedFunctionWithoutFallbackErrorStackTraceEntry,
    MissingFallbackOrReceiveErrorStackTraceEntry,
    ReturndataSizeErrorStackTraceEntry,
    NonContractAccountCalledErrorStackTraceEntry,
    CallFailedErrorStackTraceEntry,
    DirectLibraryCallErrorStackTraceEntry,
    UnrecognizedCreateErrorStackTraceEntry,
    UnrecognizedContractErrorStackTraceEntry,
    OtherExecutionErrorStackTraceEntry,
    UnmappedSolc063RevertErrorStackTraceEntry,
    ContractTooLargeErrorStackTraceEntry,
    InternalFunctionCallStackEntry,
    ContractCallRunOutOfGasError,
>;

#[allow(dead_code)]
pub type SolidityStackTrace = Vec<SolidityStackTraceEntry>;

const _: () = {
    const fn assert_to_from_napi_value<T: FromNapiValue + ToNapiValue>() {}
    assert_to_from_napi_value::<SolidityStackTraceEntry>();
};
