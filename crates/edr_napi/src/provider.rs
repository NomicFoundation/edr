mod builder;
/// Types related to provider factories.
pub mod factory;

use std::sync::Arc;

use edr_provider::InvalidRequestReason;
use edr_rpc_client::jsonrpc;
use napi::{tokio::runtime, Env, JsFunction, JsObject, Status};
use napi_derive::napi;

pub use self::{
    builder::{Builder, ProviderBuilder},
    factory::ProviderFactory,
};
use crate::{
    call_override::CallOverrideCallback,
    spec::{Response, SyncNapiSpec},
};

/// A JSON-RPC provider for Ethereum.
#[napi]
pub struct Provider {
    provider: Arc<dyn SyncProvider>,
    runtime: runtime::Handle,
    #[cfg(feature = "scenarios")]
    scenario_file: Option<napi::tokio::sync::Mutex<napi::tokio::fs::File>>,
}

impl Provider {
    /// Constructs a new instance.
    pub fn new(
        provider: Arc<dyn SyncProvider>,
        runtime: runtime::Handle,
        #[cfg(feature = "scenarios")] scenario_file: Option<
            napi::tokio::sync::Mutex<napi::tokio::fs::File>,
        >,
    ) -> Self {
        Self {
            provider,
            runtime,
            #[cfg(feature = "scenarios")]
            scenario_file,
        }
    }
}

#[napi]
impl Provider {
    #[doc = "Handles a JSON-RPC request and returns a JSON-RPC response."]
    #[napi]
    pub async fn handle_request(&self, request: serde_json::Value) -> napi::Result<Response> {
        let provider = self.provider.clone();

        #[cfg(feature = "scenarios")]
        if let Some(scenario_file) = &self.scenario_file {
            crate::scenarios::write_request(scenario_file, &request).await?;
        }

        runtime::Handle::current()
            .spawn_blocking(move || provider.handle_request(request))
            .await
            .map_err(|error| napi::Error::new(Status::GenericFailure, error.to_string()))?
    }

    #[napi(ts_return_type = "Promise<void>")]
    pub fn set_call_override_callback(
        &self,
        env: Env,
        #[napi(
            ts_arg_type = "(contract_address: Buffer, data: Buffer) => Promise<CallOverrideResult | undefined>"
        )]
        call_override_callback: JsFunction,
    ) -> napi::Result<JsObject> {
        let call_override_callback =
            CallOverrideCallback::new(&env, call_override_callback, self.runtime.clone())?;

        let provider = self.provider.clone();

        let (deferred, promise) = env.create_deferred()?;
        self.runtime.spawn_blocking(move || {
            provider.set_call_override_callback(call_override_callback);

            deferred.resolve(|_env| Ok(()));
        });

        Ok(promise)
    }

    /// Set to `true` to make the traces returned with `eth_call`,
    /// `eth_estimateGas`, `eth_sendRawTransaction`, `eth_sendTransaction`,
    /// `evm_mine`, `hardhat_mine` include the full stack and memory. Set to
    /// `false` to disable this.
    #[napi]
    pub async fn set_verbose_tracing(&self, verbose_tracing: bool) -> napi::Result<()> {
        let provider = self.provider.clone();

        self.runtime
            .spawn_blocking(move || {
                provider.set_verbose_tracing(verbose_tracing);
            })
            .await
            .map_err(|error| napi::Error::new(Status::GenericFailure, error.to_string()))
    }
}

/// Trait for a synchronous N-API provider that can be used for dynamic trait
/// objects.
pub trait SyncProvider: Send + Sync {
    /// Blocking method to handle a request.
    fn handle_request(&self, request: serde_json::Value) -> napi::Result<Response>;

    /// Set to `true` to make the traces returned with `eth_call`,
    /// `eth_estimateGas`, `eth_sendRawTransaction`, `eth_sendTransaction`,
    /// `evm_mine`, `hardhat_mine` include the full stack and memory. Set to
    /// `false` to disable this.
    fn set_call_override_callback(&self, call_override_callback: CallOverrideCallback);

    /// Set the verbose tracing flag to the provided value.
    fn set_verbose_tracing(&self, enabled: bool);
}

impl<ChainSpecT: SyncNapiSpec> SyncProvider for edr_provider::Provider<ChainSpecT> {
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
                    .map(Response::from);
            }
        };

        let response = edr_provider::Provider::handle_request(self, request);

        ChainSpecT::cast_response(response)
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
