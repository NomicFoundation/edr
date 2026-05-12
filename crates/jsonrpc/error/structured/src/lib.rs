#![warn(missing_docs)]
//! Structured JSON-RPC error types.

use edr_jsonrpc_api::RpcErrorCode;
use erased_serde::serialize_trait_object;
use serde::ser::SerializeMap as _;

/// Application-defined code for invalid input errors.
pub const INVALID_INPUT_CODE: i16 = -32000;

/// Application-defined code for EVM transaction revert errors.
pub const REVERT_CODE: i16 = 3;

/// Trait for structured JSON-RPC errors, which can be serialized and converted
/// into a JSON-RPC error response with a specific error code and tag.
pub trait RpcStructuredError:
    std::error::Error + RpcErrorCode + DynRpcStructuredErrorTag + erased_serde::Serialize
{
}

serialize_trait_object!(RpcStructuredError);

/// Wrapper around a structured JSON-RPC error, used to erase the concrete type
/// of the error while still allowing it to be serialized and converted into a
/// [`edr_jsonrpc_protocol::Error`].
pub struct DynRpcStructuredError(Box<dyn RpcStructuredError>);

impl DynRpcStructuredError {
    /// Constructs a new instance.
    pub fn new<ErrorT: 'static + RpcStructuredError>(error: ErrorT) -> Self {
        Self(Box::<dyn RpcStructuredError>::from(Box::new(error)))
    }
}

impl From<DynRpcStructuredError> for edr_jsonrpc_protocol::Error<DynRpcStructuredError> {
    fn from(value: DynRpcStructuredError) -> Self {
        Self {
            code: value.0.error_code(),
            message: value.0.to_string(),
            data: Some(value),
        }
    }
}

impl serde::Serialize for DynRpcStructuredError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("tag", self.0.error_tag())?;
        map.serialize_entry("data", &self.0)?;
        map.end()
    }
}

/// Trait for identifying the tag of a structured JSON-RPC error.
pub trait RpcStructuredErrorTag {
    /// Unique tag for this error type, used to identify the error kind without
    /// inspecting the serialized data.
    const ERROR_TAG: &'static str;
}

/// Trait for using [`RpcStructuredErrorTag`] in trait objects.
pub trait DynRpcStructuredErrorTag {
    /// Unique tag for this error type, used to identify the error kind without
    /// inspecting the serialized data.
    fn error_tag(&self) -> &'static str;
}

impl<T: RpcStructuredErrorTag> DynRpcStructuredErrorTag for T {
    fn error_tag(&self) -> &'static str {
        Self::ERROR_TAG
    }
}
