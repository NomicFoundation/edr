/// Types related to provider factories.
pub mod factory;
mod response;

use std::sync::Arc;

use edr_napi_core::provider::SyncProvider;
use edr_solidity::compiler::create_models_and_decode_bytecodes;
use napi::{
    bindgen_prelude::{Buffer, FnArgs, Function, Object, ObjectFinalize, Promise},
    tokio::runtime,
    Env, Status,
};
use napi_derive::napi;
use parking_lot::RwLock;

pub use self::factory::ProviderFactory;
use self::response::Response;
use crate::{
    async_deallocator::AsyncDeallocatorSender, call_override::CallOverrideCallback,
    contract_decoder::ContractDecoder,
};

/// A JSON-RPC provider for Ethereum.
#[napi(custom_finalize)]
pub struct Provider {
    contract_decoder: Arc<RwLock<edr_solidity::contract_decoder::ContractDecoder>>,
    provider: Arc<dyn SyncProvider>,
    runtime: runtime::Handle,
    dropped_provider_sender: AsyncDeallocatorSender<Arc<dyn SyncProvider>>,
    #[cfg(feature = "scenarios")]
    scenario_file: Option<napi::tokio::sync::Mutex<napi::tokio::fs::File>>,
}

impl Provider {
    /// Constructs a new instance.
    pub fn new(
        provider: Arc<dyn SyncProvider>,
        runtime: runtime::Handle,
        contract_decoder: Arc<RwLock<edr_solidity::contract_decoder::ContractDecoder>>,
        dropped_provider_sender: AsyncDeallocatorSender<Arc<dyn SyncProvider>>,
        #[cfg(feature = "scenarios")] scenario_file: Option<
            napi::tokio::sync::Mutex<napi::tokio::fs::File>,
        >,
    ) -> Self {
        Self {
            contract_decoder,
            provider,
            runtime,
            dropped_provider_sender,
            #[cfg(feature = "scenarios")]
            scenario_file,
        }
    }
}

#[napi]
impl Provider {
    #[doc = "Adds a compilation result to the instance."]
    #[doc = ""]
    #[doc = "For internal use only. Support for this method may be removed in the future."]
    #[napi(catch_unwind)]
    pub async fn add_compilation_result(
        &self,
        solc_version: String,
        compiler_input: serde_json::Value,
        compiler_output: serde_json::Value,
    ) -> napi::Result<()> {
        let contract_decoder = self.contract_decoder.clone();

        self.runtime
            .spawn_blocking(move || {
                let compiler_input = serde_json::from_value(compiler_input)
                    .map_err(|error| napi::Error::from_reason(error.to_string()))?;

                let compiler_output = serde_json::from_value(compiler_output)
                    .map_err(|error| napi::Error::from_reason(error.to_string()))?;

                let contracts = match create_models_and_decode_bytecodes(
                    solc_version,
                    &compiler_input,
                    &compiler_output,
                ) {
                    Ok(contracts) => contracts,
                    Err(error) => {
                        return Err(napi::Error::from_reason(format!("Contract decoder failed to be updated. Please report this to help us improve Hardhat.\n{error}")));
                    }
                };

                let mut contract_decoder = contract_decoder.write();
                for contract in contracts {
                    contract_decoder.add_contract_metadata(contract);
                }

                Ok(())
            })
            .await
            .map_err(|error| napi::Error::new(Status::GenericFailure, error.to_string()))?
    }

    #[doc = "Retrieves the instance's contract decoder."]
    #[napi(catch_unwind)]
    pub fn contract_decoder(&self) -> ContractDecoder {
        ContractDecoder::from(Arc::clone(&self.contract_decoder))
    }

    #[doc = "Handles a JSON-RPC request and returns a JSON-RPC response."]
    #[napi(catch_unwind)]
    pub async fn handle_request(&self, request: String) -> napi::Result<Response> {
        let provider = self.provider.clone();

        #[cfg(feature = "scenarios")]
        if let Some(scenario_file) = &self.scenario_file {
            crate::scenarios::write_request(scenario_file, &request).await?;
        }

        self.runtime
            .spawn_blocking(move || provider.handle_request(request))
            .await
            .map_err(|error| napi::Error::new(Status::GenericFailure, error.to_string()))?
            .map(Response::from)
    }

    #[napi(catch_unwind, ts_return_type = "Promise<void>")]
    pub fn set_call_override_callback<'env>(
        &self,
        env: &'env Env,
        #[napi(
            ts_arg_type = "(contract_address: ArrayBuffer, data: ArrayBuffer) => Promise<CallOverrideResult | undefined>"
        )]
        call_override_callback: Function<
            'env,
            FnArgs<(Buffer, Buffer)>,
            Promise<Option<crate::call_override::CallOverrideResult>>,
        >,
    ) -> napi::Result<Object<'env>> {
        let (deferred, promise) = env.create_deferred()?;

        let call_override_callback =
            match CallOverrideCallback::new(env, call_override_callback, self.runtime.clone()) {
                Ok(callback) => callback,
                Err(error) => {
                    deferred.reject(error);
                    return Ok(promise);
                }
            };

        let call_override_callback =
            Arc::new(move |address, data| call_override_callback.call_override(address, data));

        let provider = self.provider.clone();
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
    #[napi(catch_unwind)]
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

impl ObjectFinalize for Provider {
    fn finalize(self, _env: Env) -> napi::Result<()> {
        let Self {
            provider,
            dropped_provider_sender,
            ..
        } = self;

        // Off-loads deallocation to a background thread to avoid blocking the
        // JS thread (the provider may be running thread-safe functions on a
        // background thread; dropping it here would deadlock).
        dropped_provider_sender.deallocate(provider);

        Ok(())
    }
}
