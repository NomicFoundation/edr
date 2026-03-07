use edr_primitives::HashMap;
use foundry_evm_traces::CallTraceArena;

use crate::handlers::{
    error::{DynProviderError, RpcErrorCode},
    eth::ETH_GET_TRANSACTION_COUNT_METHOD,
};

pub mod error;
pub mod eth;

/// A JSON-RPC request to the provider.
pub enum RpcRequest {
    /// A single JSON-RPC request
    Single(RpcMethodCall),
    /// A batch of JSON-RPC requests
    Batch(Vec<RpcMethodCall>),
}

/// A JSON-RPC method call, consisting of the method name and parameters.
pub struct RpcMethodCall {
    pub method: String,
    pub params: serde_json::Value,
}

#[derive(Debug, thiserror::Error)]
#[error("Method {name} is not supported")]
pub struct UnsupportedMethodError {
    name: String,
}

impl RpcErrorCode for UnsupportedMethodError {
    fn error_code(&self) -> i16 {
        -32004
    }
}

pub fn default_handlers() -> HashMap<
    &'static str,
    Box<dyn Fn(serde_json::Value) -> Result<ResponseWithCallTraces, DynProviderError>>,
> {
    eth_handlers().into_iter().collect()
}

fn eth_handlers() -> [(
    &'static str,
    Box<dyn Fn(serde_json::Value) -> Result<ResponseWithCallTraces, DynProviderError>>,
); 1] {
    [(
        ETH_GET_TRANSACTION_COUNT_METHOD,
        homogenize_fallible_handler(eth::handle_get_transaction_count_request),
    )]
}

/// Transforms a typed, fallible handler into a homogeneous handler that takes
/// [`serde_json::Value`] as input and output.
pub fn homogenize_fallible_handler<
    HandlerT: 'static + Fn(ParamsT) -> Result<SuccessT, DynProviderError>,
    ParamsT: serde::de::DeserializeOwned,
    SuccessT: serde::Serialize,
>(
    handler: HandlerT,
) -> Box<dyn Fn(serde_json::Value) -> Result<ResponseWithCallTraces, DynProviderError>> {
    Box::new(move |params: serde_json::Value| {
        let deserialized_params = serde_json::from_value(params).map_err(DynProviderError::new)?;
        let success = handler(deserialized_params)?;

        let result = serde_json::to_value(success).map_err(DynProviderError::new)?;

        Ok(ResponseWithCallTraces {
            result,
            call_trace_arenas: Vec::new(),
        })
    })
}

/// Transforms a typed, fallible handler with up to one call trace into a
/// homogeneous handler that takes [`serde_json::Value`] as input and output.
pub fn homogenize_fallible_handler_with_trace<
    HandlerT: 'static + Fn(ParamsT) -> Result<(SuccessT, Option<CallTraceArena>), DynProviderError>,
    ParamsT: serde::de::DeserializeOwned,
    SuccessT: serde::Serialize,
>(
    handler: HandlerT,
) -> Box<dyn Fn(serde_json::Value) -> Result<ResponseWithCallTraces, DynProviderError>> {
    Box::new(move |params: serde_json::Value| {
        let deserialized_params = serde_json::from_value(params).map_err(DynProviderError::new)?;
        let (success, call_trace) = handler(deserialized_params)?;

        let result = serde_json::to_value(success).map_err(DynProviderError::new)?;

        Ok(ResponseWithCallTraces {
            result,
            call_trace_arenas: call_trace.into_iter().collect(),
        })
    })
}

/// Transforms a typed, fallible handler with multiple call traces into a
/// homogeneous handler that takes [`serde_json::Value`] as input and output.
pub fn homogenize_fallible_handler_with_traces<
    HandlerT: 'static + Fn(ParamsT) -> Result<(SuccessT, Vec<CallTraceArena>), DynProviderError>,
    ParamsT: serde::de::DeserializeOwned,
    SuccessT: serde::Serialize,
>(
    handler: HandlerT,
) -> Box<dyn Fn(serde_json::Value) -> Result<ResponseWithCallTraces, DynProviderError>> {
    Box::new(move |params: serde_json::Value| {
        let deserialized_params = serde_json::from_value(params).map_err(DynProviderError::new)?;
        let (success, call_trace_arenas) = handler(deserialized_params)?;

        let result = serde_json::to_value(success).map_err(DynProviderError::new)?;
        Ok(ResponseWithCallTraces {
            result,
            call_trace_arenas,
        })
    })
}

/// Transforms a typed, infallible handler into a homogeneous handler that takes
/// [`serde_json::Value`] as input and output.
pub fn homogenize_infallible_handler<
    HandlerT: 'static + Fn(ParamsT) -> SuccessT,
    ParamsT: serde::de::DeserializeOwned,
    SuccessT: serde::Serialize,
>(
    handler: HandlerT,
) -> Box<dyn Fn(serde_json::Value) -> Result<ResponseWithCallTraces, DynProviderError>> {
    Box::new(move |params: serde_json::Value| {
        let deserialized_params = serde_json::from_value(params).map_err(DynProviderError::new)?;
        let success = handler(deserialized_params);

        let result = serde_json::to_value(success).map_err(DynProviderError::new)?;

        Ok(ResponseWithCallTraces {
            result,
            call_trace_arenas: Vec::new(),
        })
    })
}
