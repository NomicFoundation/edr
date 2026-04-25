#![warn(missing_docs)]
//! Structured JSON-RPC error types.

use edr_jsonrpc_api::RpcErrorData;

/// Application-defined code for invalid input errors.
pub const INVALID_INPUT_CODE: i16 = -32000;

/// Application-defined code for EVM transaction revert errors.
pub const REVERT_CODE: i16 = 3;

// impl RpcStructuredError {
//     pub fn new<T: serde::Serialize + RpcStructuredErrorTag>(
//         error: T,
//     ) -> Result<Self, serde_json::Error> {
//         Ok(Self {
//             tag: T::ERROR_TAG,
//             data: serde_json::to_value(error)?,
//         })
//     }
// }

/// Trait for identifying the tag of a structured JSON-RPC error.
pub trait RpcStructuredErrorTag {
    /// Unique tag for this error type, used to identify the error kind without
    /// inspecting the serialized data.
    const ERROR_TAG: &'static str;
}

impl<ErrorT: RpcStructuredErrorTag + serde::Serialize> RpcErrorData for ErrorT {
    fn error_data(&self) -> Option<serde_json::Value> {
        let data = serde_json::json!({
            "tag": Self::ERROR_TAG,
            "data": self,
        });

        Some(data)
    }
}
