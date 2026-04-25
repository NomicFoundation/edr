#![warn(missing_docs)]
//! Traits for exposing JSON-RPC error information from error types.

/// Trait for retrieving the JSON-RPC error code from an error type.
pub trait RpcErrorCode {
    /// Returns the JSON-RPC error code.
    fn error_code(&self) -> i16;
}

/// Trait for retrieving the JSON-RPC error data from an error type.
pub trait RpcErrorData {
    /// Returns the JSON-RPC error data, if any.
    fn error_data(&self) -> Option<serde_json::Value> {
        None
    }
}
