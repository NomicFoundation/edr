//! Defines handlers for JSON-RPC methods.

use core::fmt;

use edr_primitives::HashMap;
use foundry_evm_traces::CallTraceArena;

use crate::{
    handlers::{
        error::{DynProviderError, RpcErrorCode, RpcTypedError},
        eth::ETH_GET_TRANSACTION_COUNT_METHOD,
    },
    time::TimeSinceEpoch,
    ProviderData, ResponseWithCallTraces, SyncProviderSpec,
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

impl RpcRequest {
    /// Constructs a new instance from a single [`RpcMethodCall`].
    pub fn with_single(method: RpcMethodCall) -> Self {
        Self::Single(method)
    }
}

// Custom deserializer instead of using `#[serde(untagged)]` as the latter hides
// custom error messages which are important to propagate to users.
impl<'deserializer> serde::de::Deserialize<'deserializer> for RpcRequest {
    fn deserialize<DeserializerT>(deserializer: DeserializerT) -> Result<Self, DeserializerT::Error>
    where
        DeserializerT: serde::Deserializer<'deserializer>,
    {
        struct SingleOrBatchRequestVisitor;

        impl<'de> serde::de::Visitor<'de> for SingleOrBatchRequestVisitor {
            type Value = RpcRequest;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("single or batch request")
            }

            fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                // Forward to deserializer of `Vec<MethodInvocation>`
                Ok(RpcRequest::Batch(serde::de::Deserialize::deserialize(
                    serde::de::value::SeqAccessDeserializer::new(seq),
                )?))
            }

            fn visit_map<M>(self, map: M) -> Result<RpcRequest, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                // Forward to deserializer of `MethodInvocation`
                Ok(RpcRequest::with_single(
                    serde::de::Deserialize::deserialize(
                        serde::de::value::MapAccessDeserializer::new(map),
                    )?,
                ))
            }
        }

        deserializer.deserialize_any(SingleOrBatchRequestVisitor)
    }
}

/// A JSON-RPC method call, consisting of the method name and parameters.
#[derive(serde::Deserialize)]
pub struct RpcMethodCall {
    pub method: String,
    pub params: Option<serde_json::Value>,
}

impl RpcMethodCall {
    /// Constructs a new instance from the given method name and parameters.
    pub fn with_params<ParamsT: serde::Serialize>(
        method: &str,
        params: ParamsT,
    ) -> Result<Self, serde_json::Error> {
        let params = serde_json::to_value(params)?;

        Ok(Self {
            method: method.to_owned(),
            params: Some(params),
        })
    }

    /// Constructs a new instance from the given method name and no parameters.
    pub fn without_params(method: &str) -> Self {
        Self {
            method: method.to_owned(),
            params: None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Invalid Request")]
pub struct InvalidRequestError;

impl RpcErrorCode for InvalidRequestError {
    fn error_code(&self) -> i16 {
        -32600
    }
}

impl RpcTypedError for InvalidRequestError {
    fn error_tag(&self) -> &'static str {
        "INVALID_REQUEST"
    }

    fn error_data(&self) -> Option<serde_json::Value> {
        None
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Method {method} is not supported")]
pub struct UnsupportedMethodError {
    pub method: String,
}

impl UnsupportedMethodError {
    pub const ERROR_TAG: &'static str = "UNSUPPORTED_METHOD";
}

impl RpcErrorCode for UnsupportedMethodError {
    fn error_code(&self) -> i16 {
        -32004
    }
}

impl RpcTypedError for UnsupportedMethodError {
    fn error_tag(&self) -> &'static str {
        UnsupportedMethodError::ERROR_TAG
    }

    fn error_data(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({ "method": self.method }))
    }
}

pub fn default_handlers<ChainSpecT: SyncProviderSpec<TimerT>, TimerT: Clone + TimeSinceEpoch>(
) -> HashMap<
    &'static str,
    Box<
        dyn Fn(
            &mut ProviderData<ChainSpecT, TimerT>,
            serde_json::Value,
        ) -> Result<ResponseWithCallTraces, DynProviderError>
            + Send
            + Sync,
    >,
> {
    eth_handlers().into_iter().collect()
}

fn eth_handlers<ChainSpecT: SyncProviderSpec<TimerT>, TimerT: Clone + TimeSinceEpoch>() -> [(
    &'static str,
    Box<
        dyn Fn(
            &mut ProviderData<ChainSpecT, TimerT>,
            serde_json::Value,
        ) -> Result<ResponseWithCallTraces, DynProviderError>
            + Send
            + Sync,
    >,
); 1] {
    [(
        ETH_GET_TRANSACTION_COUNT_METHOD,
        homogenize_fallible_handler(eth::handle_get_transaction_count_request),
    )]
}

/// Transforms a typed, fallible handler into a homogeneous handler that takes
/// [`serde_json::Value`] as input and output.
pub fn homogenize_fallible_handler<
    ChainSpecT: SyncProviderSpec<TimerT>,
    HandlerT: 'static
        + Send
        + Sync
        + Fn(&mut ProviderData<ChainSpecT, TimerT>, ParamsT) -> Result<SuccessT, DynProviderError>,
    ParamsT: serde::de::DeserializeOwned,
    SuccessT: serde::Serialize,
    TimerT: Clone + TimeSinceEpoch,
>(
    handler: HandlerT,
) -> Box<
    dyn Fn(
        &mut ProviderData<ChainSpecT, TimerT>,
        serde_json::Value,
    ) -> Result<ResponseWithCallTraces, DynProviderError>
        + Send
        + Sync,
> {
    Box::new(
        move |data: &mut ProviderData<ChainSpecT, TimerT>, params: serde_json::Value| {
            let deserialized_params =
                serde_json::from_value(params).map_err(DynProviderError::new)?;
            let success = handler(data, deserialized_params)?;

            let result = serde_json::to_value(success).map_err(DynProviderError::new)?;

            Ok(ResponseWithCallTraces {
                result,
                call_trace_arenas: Vec::new(),
            })
        },
    )
}

/// Transforms a typed, fallible handler with up to one call trace into a
/// homogeneous handler that takes [`serde_json::Value`] as input and output.
pub fn homogenize_fallible_handler_with_trace<
    ChainSpecT: SyncProviderSpec<TimerT>,
    HandlerT: 'static
        + Send
        + Sync
        + Fn(
            &mut ProviderData<ChainSpecT, TimerT>,
            ParamsT,
        ) -> Result<(SuccessT, Option<CallTraceArena>), DynProviderError>,
    ParamsT: serde::de::DeserializeOwned,
    SuccessT: serde::Serialize,
    TimerT: Clone + TimeSinceEpoch,
>(
    handler: HandlerT,
) -> Box<
    dyn Fn(
        &mut ProviderData<ChainSpecT, TimerT>,
        serde_json::Value,
    ) -> Result<ResponseWithCallTraces, DynProviderError>
        + Send
        + Sync,
> {
    Box::new(
        move |data: &mut ProviderData<ChainSpecT, TimerT>, params: serde_json::Value| {
            let deserialized_params =
                serde_json::from_value(params).map_err(DynProviderError::new)?;

            let (success, call_trace) = handler(data, deserialized_params)?;

            let result = serde_json::to_value(success).map_err(DynProviderError::new)?;

            Ok(ResponseWithCallTraces {
                result,
                call_trace_arenas: call_trace.into_iter().collect(),
            })
        },
    )
}

/// Transforms a typed, fallible handler with multiple call traces into a
/// homogeneous handler that takes [`serde_json::Value`] as input and output.
pub fn homogenize_fallible_handler_with_traces<
    ChainSpecT: SyncProviderSpec<TimerT>,
    HandlerT: 'static
        + Fn(
            &mut ProviderData<ChainSpecT, TimerT>,
            ParamsT,
        ) -> Result<(SuccessT, Vec<CallTraceArena>), DynProviderError>,
    ParamsT: serde::de::DeserializeOwned,
    SuccessT: serde::Serialize,
    TimerT: Clone + TimeSinceEpoch,
>(
    handler: HandlerT,
) -> Box<
    dyn Fn(
        &mut ProviderData<ChainSpecT, TimerT>,
        serde_json::Value,
    ) -> Result<ResponseWithCallTraces, DynProviderError>,
> {
    Box::new(
        move |data: &mut ProviderData<ChainSpecT, TimerT>, params: serde_json::Value| {
            let deserialized_params =
                serde_json::from_value(params).map_err(DynProviderError::new)?;

            let (success, call_trace_arenas) = handler(data, deserialized_params)?;

            let result = serde_json::to_value(success).map_err(DynProviderError::new)?;
            Ok(ResponseWithCallTraces {
                result,
                call_trace_arenas,
            })
        },
    )
}

/// Transforms a typed, infallible handler into a homogeneous handler that takes
/// [`serde_json::Value`] as input and output.
pub fn homogenize_infallible_handler<
    ChainSpecT: SyncProviderSpec<TimerT>,
    HandlerT: 'static + Fn(&mut ProviderData<ChainSpecT, TimerT>, ParamsT) -> SuccessT,
    ParamsT: serde::de::DeserializeOwned,
    SuccessT: serde::Serialize,
    TimerT: Clone + TimeSinceEpoch,
>(
    handler: HandlerT,
) -> Box<
    dyn Fn(
        &mut ProviderData<ChainSpecT, TimerT>,
        serde_json::Value,
    ) -> Result<ResponseWithCallTraces, DynProviderError>,
> {
    Box::new(
        move |data: &mut ProviderData<ChainSpecT, TimerT>, params: serde_json::Value| {
            let deserialized_params =
                serde_json::from_value(params).map_err(DynProviderError::new)?;

            let success = handler(data, deserialized_params);

            let result = serde_json::to_value(success).map_err(DynProviderError::new)?;

            Ok(ResponseWithCallTraces {
                result,
                call_trace_arenas: Vec::new(),
            })
        },
    )
}

#[cfg(test)]
mod tests {
    use anyhow::Context;

    use super::*;

    #[test]
    fn deserialize_single_request() -> anyhow::Result<()> {
        let json = r#"{
            "jsonrpc": "2.0",
            "method": "eth_getBalance",
            "params": ["0x407d73d8a49eeb85d32cf465507dd71d507100c1", "latest"],
            "id": 1
        }"#;
        let request: RpcRequest = serde_json::from_str(json)?;
        assert!(matches!(request, RpcRequest::Single(..)));
        Ok(())
    }

    #[test]
    fn deserialize_batch_request() -> anyhow::Result<()> {
        let json = r#"[
            {
                "jsonrpc": "2.0",
                "method": "eth_blockNumber",
                "params": [],
                "id": 1
            },
            {
                "jsonrpc": "2.0",
                "method": "eth_getTransactionByHash",
                "params": ["0x3f07a9c83155594c000642e7d60e8a8a00038d03e9849171a05ed0e2d47acbb3"],
                "id": 2
            }
        ]"#;
        let request: RpcRequest = serde_json::from_str(json)?;
        assert!(matches!(request, RpcRequest::Batch(_)));
        Ok(())
    }

    #[test]
    fn deserialize_string_instead_of_request() -> anyhow::Result<()> {
        let s = "foo";
        let json = format!(r#""{s}""#);

        let result: Result<RpcRequest, _> = serde_json::from_str(&json);

        let error_message = result.err().context("result is error")?.to_string();
        assert!(error_message.contains(s));

        Ok(())
    }
}
