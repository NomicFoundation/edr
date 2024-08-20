mod config;
mod factory;
/// Types for the L1 chain type.
pub mod l1;

use std::sync::Arc;

use edr_eth::chain_spec::L1ChainSpec;
use edr_provider::{time::CurrentTime, InvalidRequestReason};
use edr_rpc_eth::jsonrpc;
use napi::{tokio::runtime, Either, Env, JsFunction, JsObject, Status};
use napi_derive::napi;

pub use self::{
    config::ProviderConfig,
    factory::{ProviderFactory, SyncProviderFactory},
};
use crate::{
    call_override::CallOverrideCallback,
    context::EdrContext,
    logger::{Logger, LoggerConfig},
    subscribe::SubscriberCallback,
    trace::RawTrace,
};

pub trait SyncProvider {
    fn handle_request(&self, request: serde_json::Value) -> napi::Result<Response>;
}

impl SyncProvider for edr_provider::Provider<L1ChainSpec> {
    fn handle_request(&self, request: serde_json::Value) -> napi::Result<Response> {
        let method_name = request.get("method").and_then(serde_json::Value::as_str);

        let request = match serde_json::from_value(request) {
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
                        let json_request = request.to_string();
                        napi::Error::new(
                            Status::InvalidArg,
                            format!("Invalid JSON `{json_request}` due to: {error}"),
                        )
                    })
                    .map(|json_response| Response {
                        solidity_trace: None,
                        json: json_response,
                        traces: Vec::new(),
                    });
            }
        };

        let mut response = runtime::Handle::current()
            .spawn_blocking(move || provider.handle_request(request))
            .await
            .map_err(|e| napi::Error::new(Status::GenericFailure, e.to_string()))?;
        self.handle_request(request)
            .map_err(|error| napi::Error::new(Status::GenericFailure, error.to_string()))
    }
}

/// A JSON-RPC provider for Ethereum.
#[napi]
pub struct Provider {
    provider: Arc<dyn SyncProvider>,
    runtime: runtime::Handle,
    #[cfg(feature = "scenarios")]
    scenario_file: Option<napi::tokio::sync::Mutex<napi::tokio::fs::File>>,
}

impl Provider {
    pub fn new(provider: Arc<dyn SyncProvider>, runtime: runtime::Handle) -> Self {
        Self {
            provider,
            runtime,
            #[cfg(feature = "scenarios")]
            scenario_file: None,
        }
    }
}

#[napi]
impl Provider {
    #[doc = "Handles a JSON-RPC request and returns a JSON-RPC response."]
    #[napi]
    pub async fn handle_request(&self, json_request: String) -> napi::Result<Response> {
        let provider = self.provider.clone();
        let request = match serde_json::from_str(&json_request) {
            Ok(request) => request,
            Err(error) => {
                let message = error.to_string();
                let reason = InvalidRequestReason::new(&json_request, &message);

                // HACK: We need to log failed deserialization attempts when they concern input
                // validation.
                if let Some((method_name, provider_error)) = reason.provider_error() {
                    // Ignore potential failure of logging, as returning the original error is more
                    // important
                    let _result = runtime::Handle::current()
                        .spawn_blocking(move || {
                            provider.log_failed_deserialization(&method_name, &provider_error)
                        })
                        .await
                        .map_err(|error| {
                            napi::Error::new(Status::GenericFailure, error.to_string())
                        })?;
                }

                let data = serde_json::from_str(&json_request).ok();
                let response = jsonrpc::ResponseData::<()>::Error {
                    error: jsonrpc::Error {
                        code: reason.error_code(),
                        message: reason.error_message(),
                        data,
                    },
                };

                return serde_json::to_string(&response)
                    .map_err(|error| {
                        napi::Error::new(
                            Status::InvalidArg,
                            format!("Invalid JSON `{json_request}` due to: {error}"),
                        )
                    })
                    .map(|json| Response {
                        solidity_trace: None,
                        data: Either::A(json),
                        traces: Vec::new(),
                    });
            }
        };

        #[cfg(feature = "scenarios")]
        if let Some(scenario_file) = &self.scenario_file {
            crate::scenarios::write_request(scenario_file, &request).await?;
        }

        let mut response = runtime::Handle::current()
            .spawn_blocking(move || provider.handle_request(request))
            .await
            .map_err(|e| napi::Error::new(Status::GenericFailure, e.to_string()))?;

        // We can take the solidity trace as it won't be used for anything else
        let solidity_trace = response.as_mut().err().and_then(|error| {
            if let edr_provider::ProviderError::TransactionFailed(failure) = error {
                if matches!(
                    failure.failure.reason,
                    edr_provider::TransactionFailureReason::OutOfGas(_)
                ) {
                    None
                } else {
                    Some(Arc::new(std::mem::take(
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
                traces: traces.into_iter().map(Arc::new).collect(),
            })
    }

    #[napi(ts_return_type = "void")]
    pub fn set_call_override_callback(
        &self,
        env: Env,
        #[napi(
            ts_arg_type = "(contract_address: Buffer, data: Buffer) => Promise<CallOverrideResult | undefined>"
        )]
        call_override_callback: JsFunction,
    ) -> napi::Result<()> {
        let provider = self.provider.clone();

        let call_override_callback =
            CallOverrideCallback::new(&env, call_override_callback, self.runtime.clone())?;
        let call_override_callback =
            Arc::new(move |address, data| call_override_callback.call_override(address, data));

        provider.set_call_override_callback(Some(call_override_callback));

        Ok(())
    }

    /// Set to `true` to make the traces returned with `eth_call`,
    /// `eth_estimateGas`, `eth_sendRawTransaction`, `eth_sendTransaction`,
    /// `evm_mine`, `hardhat_mine` include the full stack and memory. Set to
    /// `false` to disable this.
    #[napi(ts_return_type = "void")]
    pub fn set_verbose_tracing(&self, verbose_tracing: bool) {
        self.provider.set_verbose_tracing(verbose_tracing);
    }
}

#[napi]
pub struct Response {
    // N-API is known to be slow when marshalling `serde_json::Value`s, so we try to return a
    // `String`. If the object is too large to be represented as a `String`, we return a `Buffer`
    // instead.
    data: Either<String, serde_json::Value>,
    /// When a transaction fails to execute, the provider returns a trace of the
    /// transaction.
    solidity_trace: Option<Arc<edr_evm::trace::Trace<L1ChainSpec>>>,
    /// This may contain zero or more traces, depending on the (batch) request
    traces: Vec<Arc<edr_evm::trace::Trace<L1ChainSpec>>>,
}

#[napi]
impl Response {
    /// Returns the response data as a JSON string or a JSON object.
    #[napi(getter)]
    pub fn data(&self) -> Either<String, serde_json::Value> {
        self.data.clone()
    }

    #[napi(getter)]
    pub fn solidity_trace(&self) -> Option<RawTrace> {
        self.solidity_trace
            .as_ref()
            .map(|trace| RawTrace::new(trace.clone()))
    }

    #[napi(getter)]
    pub fn traces(&self) -> Vec<RawTrace> {
        self.traces
            .iter()
            .map(|trace| RawTrace::new(trace.clone()))
            .collect()
    }
}
