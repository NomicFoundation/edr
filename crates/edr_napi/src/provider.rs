/// Types related to provider factories.
pub mod factory;
mod response;

use std::sync::Arc;

use edr_napi_core::provider::SyncProvider;
use edr_solidity::compiler::create_models_and_decode_bytecodes;
use napi::{
    bindgen_prelude::{FnArgs, Function, Object, Promise, Uint8Array},
    tokio::runtime,
    Env, Status,
};
use napi_derive::napi;
use parking_lot::{Mutex, RwLock};

pub use self::factory::ProviderFactory;
use self::response::Response;
use crate::{call_override::CallOverrideCallback, contract_decoder::ContractDecoder};

/// A JSON-RPC provider for Ethereum.
///
/// The inner [`Arc<dyn SyncProvider>`] is held inside a `Mutex<Option<_>>` so
/// that [`Provider::close`] can drop it explicitly. Dropping it triggers the
/// cleanup cascade — `IntervalMiner::Drop` cancels its background task,
/// `ProviderData` releases the held `Box<dyn SyncLogger>` /
/// `Box<dyn SyncSubscriberCallback>` / `Option<Arc<dyn SyncCallOverride>>`,
/// each of which releases its underlying `napi_threadsafe_function` handles
/// while the napi env is still healthy. See
/// `/workspace/napi-rs-v3-tsfn-shutdown-investigation.md` for the full story.
#[napi]
pub struct Provider {
    contract_decoder: Arc<RwLock<edr_solidity::contract_decoder::ContractDecoder>>,
    provider: Mutex<Option<Arc<dyn SyncProvider>>>,
    runtime: runtime::Handle,
    #[cfg(feature = "scenarios")]
    scenario_file: Option<napi::tokio::sync::Mutex<napi::tokio::fs::File>>,
}

impl Provider {
    /// Constructs a new instance.
    pub fn new(
        provider: Arc<dyn SyncProvider>,
        runtime: runtime::Handle,
        contract_decoder: Arc<RwLock<edr_solidity::contract_decoder::ContractDecoder>>,
        #[cfg(feature = "scenarios")] scenario_file: Option<
            napi::tokio::sync::Mutex<napi::tokio::fs::File>,
        >,
    ) -> Self {
        Self {
            contract_decoder,
            provider: Mutex::new(Some(provider)),
            runtime,
            #[cfg(feature = "scenarios")]
            scenario_file,
        }
    }

    /// Returns a clone of the inner [`SyncProvider`], or an error if
    /// [`Provider::close`] has been called.
    fn inner(&self) -> napi::Result<Arc<dyn SyncProvider>> {
        self.provider
            .lock()
            .as_ref()
            .map(Arc::clone)
            .ok_or_else(|| napi::Error::from_reason("Provider has been closed"))
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
        let provider = self.inner()?;

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
        // `ts_arg_type` declares `ArrayBuffer` for HH2 backwards-compat: that
        // version's `provider.ts` types this callback as
        // `(address: ArrayBuffer, data: ArrayBuffer) => ...` and would fail
        // type-check against `Uint8Array`. The runtime value is actually a
        // `Uint8Array` — napi-rs v3's `ArrayBuffer<'env>` carries a lifetime
        // and TSFN `FnArgs` is `'static`-bound (napi-rs v3 `function.rs` line
        // 311), so producing an `ArrayBuffer` inside the TSFN call site is
        // not possible without an unsafe-FFI shim.
        //
        // HH2's body is `Buffer.from(address)`, which accepts both shapes
        // identically, so the type/runtime skew is invisible to it. Any new
        // consumer should treat this argument as a typed-array view: methods
        // like `Buffer.from(x)` and `new Uint8Array(x)` work; ArrayBuffer-
        // specific operations (`new DataView(x)` with no offset, ArrayBuffer
        // `.slice(start, end)` semantics) would silently behave like
        // Uint8Array. Same caveat applies to `decodeConsoleLogInputsCallback`
        // in `logger.rs`.
        #[napi(
            ts_arg_type = "(contract_address: ArrayBuffer, data: ArrayBuffer) => Promise<CallOverrideResult | undefined>"
        )]
        call_override_callback: Function<
            'env,
            FnArgs<(Uint8Array, Uint8Array)>,
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

        let provider = match self.inner() {
            Ok(provider) => provider,
            Err(error) => {
                deferred.reject(error);
                return Ok(promise);
            }
        };
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
        let provider = self.inner()?;

        self.runtime
            .spawn_blocking(move || {
                provider.set_verbose_tracing(verbose_tracing);
            })
            .await
            .map_err(|error| napi::Error::new(Status::GenericFailure, error.to_string()))
    }

    /// Releases the underlying provider and all associated resources
    /// (subscription / call-override / logger threadsafe-functions, the
    /// interval-mining background task) synchronously, while the napi env
    /// is still healthy. Any subsequent method call on this `Provider`
    /// returns a `"Provider has been closed"` error.
    ///
    /// Calling `close()` is the supported way to release a `Provider`
    /// without relying on JS GC + Node env teardown to fire `napi_finalize`
    /// at the right time. The underlying `napi_threadsafe_function`
    /// shutdown path has a documented data-race / use-after-free during
    /// Node env cleanup ([`nodejs/node#55706`][1]; fix in
    /// [`nodejs/node#55877`][2] is in Node 25 only, not backported to
    /// Node 22 or 20). Without an explicit `close()` consumers rely on
    /// V8 GC heuristics — empirically reliable for typical Hardhat
    /// workloads but fragile for programmatic multi-provider patterns
    /// and prone to surfacing as macOS-15 atexit crashes (see
    /// `/workspace/napi-rs-v3-tsfn-shutdown-investigation.md`).
    ///
    /// `close()` is idempotent: calling it twice is a no-op the second
    /// time. Methods called after `close()` return an error rather than
    /// silently no-op, to surface the lifecycle violation.
    ///
    /// [1]: https://github.com/nodejs/node/issues/55706
    /// [2]: https://github.com/nodejs/node/commit/350b0ea895
    #[napi(catch_unwind)]
    pub async fn close(&self) -> napi::Result<()> {
        // Take the Arc out of the Option, dropping our reference here.
        let inner = self.provider.lock().take();

        // Move the Arc into spawn_blocking so its drop fires on a tokio
        // worker thread. `IntervalMiner::Drop` calls `block_in_place` +
        // `Handle::block_on(background_task)` to cancel and join the
        // interval-mining task, and `block_in_place` is only valid in a
        // tokio runtime context — calling drop directly from the JS
        // thread would panic.
        //
        // Note: in-flight `spawn_blocking` tasks from concurrent
        // `handle_request` / `set_call_override_callback` invocations
        // hold their own Arc clones. Our drop here reduces the Arc
        // refcount from N to N-1; the cleanup cascade fires only when
        // those tasks finish (microseconds in steady state). Not a
        // correctness issue.
        if let Some(provider) = inner {
            self.runtime
                .spawn_blocking(move || drop(provider))
                .await
                .map_err(|error| napi::Error::new(Status::GenericFailure, error.to_string()))?;
        }

        Ok(())
    }
}
