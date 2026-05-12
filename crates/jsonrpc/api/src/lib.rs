#![warn(missing_docs)]
//! Traits for exposing JSON-RPC error information from error types.

/// Trait for retrieving the JSON-RPC error code from an error type.
pub trait RpcErrorCode {
    /// Returns the JSON-RPC error code.
    fn error_code(&self) -> i16;
}
