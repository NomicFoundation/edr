mod builder;
mod config;

use std::sync::Arc;

use edr_eth::chain_spec::L1ChainSpec;
use edr_provider::InvalidRequestReason;
use edr_rpc_client::jsonrpc;
use napi::{Either, Status};
use napi_derive::napi;

pub use self::{
    builder::Builder,
    config::{Config, HardforkActivation},
};
use crate::{
    call_override::CallOverrideCallback, logger::LoggerConfig, subscription::SubscriptionConfig,
    trace::RawTrace,
};

#[napi]
pub struct Response {
    // N-API is known to be slow when marshalling `serde_json::Value`s, so we try to return a
    // `String`. If the object is too large to be represented as a `String`, we return a `Buffer`
    // instead.
    data: Either<String, serde_json::Value>,
    /// When a transaction fails to execute, the provider returns a trace of the
    /// transaction.
    solidity_trace: Option<RawTrace>,
    /// This may contain zero or more traces, depending on the (batch) request
    traces: Vec<RawTrace>,
}

#[napi]
impl Response {
    #[doc = "Returns the response data as a JSON string or a JSON object."]
    #[napi(getter)]
    pub fn data(&self) -> Either<String, serde_json::Value> {
        self.data.clone()
    }

    #[doc = "Returns the Solidity trace of the transaction that failed to execute, if any."]
    #[napi(getter)]
    pub fn solidity_trace(&self) -> Option<RawTrace> {
        self.solidity_trace.clone()
    }

    #[doc = "Returns the raw traces of executed contracts. This maybe contain zero or more traces."]
    #[napi(getter)]
    pub fn traces(&self) -> Vec<RawTrace> {
        self.traces.clone()
    }
}

pub trait SyncProvider: Send + Sync {
    fn handle_request(&self, request: serde_json::Value) -> napi::Result<Response>;

    fn set_call_override_callback(&self, call_override_callback: CallOverrideCallback);

    fn set_verbose_tracing(&self, enabled: bool);
}

/// Trait for creating a new provider using the builder pattern.
pub trait SyncProviderFactory: Send + Sync {
    /// Creates a `ProviderBuilder` that.
    fn create_provider_builder(
        &self,
        env: &napi::Env,
        provider_config: Config,
        logger_config: LoggerConfig,
        subscription_config: SubscriptionConfig,
    ) -> napi::Result<Box<dyn Builder>>;
}

impl SyncProvider for edr_provider::Sequential<L1ChainSpec> {
    fn handle_request(&self, request: serde_json::Value) -> napi::Result<Response> {
        let method_name = request.get("method").and_then(serde_json::Value::as_str);

        let request = match serde_json::from_value(request.clone()) {
            Ok(request) => request,
            Err(error) => {
                let message = error.to_string();
                let reason = InvalidRequestReason::new(method_name, &message);

                // HACK: We need to log failed deserialization attempts when they concern input
                // validation.
                if let Some((method_name, provider_error)) = reason.provider_error() {
                    // Ignore potential failure of logging, as returning the original error is more
                    // important
                    let _result = self.log_failed_deserialization(method_name, &provider_error);
                }

                let response = jsonrpc::ResponseData::<()>::Error {
                    error: jsonrpc::Error {
                        code: reason.error_code(),
                        message: reason.error_message(),
                        data: Some(request),
                    },
                };

                return serde_json::to_string(&response)
                    .map_err(|error| {
                        napi::Error::new(
                            Status::Unknown,
                            format!("Failed to serialize response due to: {error}"),
                        )
                    })
                    .map(|json| Response {
                        solidity_trace: None,
                        data: Either::A(json),
                        traces: Vec::new(),
                    });
            }
        };

        // #[cfg(feature = "scenarios")]
        // if let Some(scenario_file) = &self.scenario_file {
        //     crate::scenarios::write_request(scenario_file, &request).await?;
        // }

        let mut response = self.handle_request(request);

        // We can take the solidity trace as it won't be used for anything else
        let solidity_trace = response.as_mut().err().and_then(|error| {
            if let edr_provider::ProviderError::TransactionFailed(failure) = error {
                if matches!(
                    failure.failure.reason,
                    edr_provider::TransactionFailureReason::OutOfGas(_)
                ) {
                    None
                } else {
                    Some(RawTrace::from(std::mem::take(
                        &mut failure.failure.solidity_trace,
                    )))
                }
            } else {
                None
            }
        });

        // We can take the traces as they won't be used for anything else
        let traces = match &mut response {
            Ok(response) => std::mem::take(&mut response.traces),
            Err(edr_provider::ProviderError::TransactionFailed(failure)) => {
                std::mem::take(&mut failure.traces)
            }
            Err(_) => Vec::new(),
        };

        let response = jsonrpc::ResponseData::from(response.map(|response| response.result));

        serde_json::to_string(&response)
            .and_then(|json| {
                // We experimentally determined that 500_000_000 was the maximum string length
                // that can be returned without causing the error:
                //
                // > Failed to convert rust `String` into napi `string`
                //
                // To be safe, we're limiting string lengths to half of that.
                const MAX_STRING_LENGTH: usize = 250_000_000;

                if json.len() <= MAX_STRING_LENGTH {
                    Ok(Either::A(json))
                } else {
                    serde_json::to_value(response).map(Either::B)
                }
            })
            .map_err(|error| napi::Error::new(Status::GenericFailure, error.to_string()))
            .map(|data| Response {
                solidity_trace,
                data,
                traces: traces.into_iter().map(RawTrace::from).collect(),
            })
    }

    fn set_call_override_callback(&self, call_override_callback: CallOverrideCallback) {
        let call_override_callback =
            Arc::new(move |address, data| call_override_callback.call_override(address, data));

        self.set_call_override_callback(Some(call_override_callback));
    }

    fn set_verbose_tracing(&self, enabled: bool) {
        self.set_verbose_tracing(enabled);
    }
}
