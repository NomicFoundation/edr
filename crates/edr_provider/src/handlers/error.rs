//! Types for error handling in handlers.

pub const INVALID_INPUT: i16 = -32000;
pub const INTERNAL_ERROR: i16 = -32603;
pub const INVALID_PARAMS: i16 = -32602;

/// Trait for retrieving the JSON-RPC error code from an error type.
pub trait RpcErrorCode {
    /// Returns the JSON-RPC error code.
    fn error_code(&self) -> i16;
}

pub trait RpcError: RpcErrorCode + std::error::Error {}

impl<ErrorT: RpcErrorCode + std::error::Error> RpcError for ErrorT {}

/// Wrapper around `Box<dyn std::error::Error` to allow implementation of
/// `std::error::Error`.
// This is required because of:
// <https://stackoverflow.com/questions/65151237/why-doesnt-boxdyn-error-implement-error#65151318>
#[derive(Debug)]
pub struct DynProviderError(Box<dyn RpcError + Send + Sync>);

impl DynProviderError {
    /// Constructs a new instance.
    pub fn new<ErrorT: 'static + RpcError + Send + Sync>(error: ErrorT) -> Self {
        Self(Box::new(error))
    }
}

impl core::fmt::Display for DynProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::error::Error for DynProviderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}
