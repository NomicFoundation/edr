mod config;
mod factory;
/// Types for the L1 chain type.
pub mod l1;
mod optimism;

use std::sync::Arc;

use edr_napi_core::{
    call_override::CallOverrideCallback,
    provider::{Response, SyncProvider},
};
use napi::{tokio::runtime, Env, JsFunction, JsObject, Status};
use napi_derive::napi;

pub use self::{config::ProviderConfig, factory::ProviderFactory};

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
