/// Types related to provider factories.
pub mod factory;
mod response;

use std::{fmt::Formatter, sync::Arc};

use edr_napi_core::provider::SyncProvider;
use edr_provider::{time::CurrentTime, InvalidRequestReason};
use edr_rpc_eth::jsonrpc;
use edr_solidity::contract_decoder::{ContractDecoder, NestedTraceDecoder};
use napi::{
    bindgen_prelude::Uint8Array, tokio::runtime, Either, Env, JsFunction, JsObject, Status,
};
use napi_derive::napi;

pub use self::factory::ProviderFactory;
use self::response::Response;
use crate::call_override::CallOverrideCallback;

/// A JSON-RPC provider for Ethereum.
#[napi]
pub struct Provider {
    contract_decoder: Arc<ContractDecoder>,
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
        contract_decoder: Arc<ContractDecoder>,
        #[cfg(feature = "scenarios")] scenario_file: Option<
            napi::tokio::sync::Mutex<napi::tokio::fs::File>,
        >,
    ) -> Self {
        Self {
            contract_decoder,
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
    pub async fn handle_request(&self, request: String) -> napi::Result<Response> {
        let provider = self.provider.clone();

        #[cfg(feature = "scenarios")]
        if let Some(scenario_file) = &self.scenario_file {
            crate::scenarios::write_request(scenario_file, &request).await?;
        }

        let contract_decoder = Arc::clone(&self.contract_decoder);

        self.runtime
            .spawn_blocking(move || provider.handle_request(request, contract_decoder))
            .await
            .map_err(|error| napi::Error::new(Status::GenericFailure, error.to_string()))?
            .map(Response::from)
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

        let call_override_callback =
            Arc::new(move |address, data| call_override_callback.call_override(address, data));

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
    #[napi(ts_return_type = "void")]
    pub fn set_verbose_tracing(&self, verbose_tracing: bool) {
        self.runtime
            .spawn_blocking(move || {
                provider.set_verbose_tracing(verbose_tracing);
            })
            .await
            .map_err(|error| napi::Error::new(Status::GenericFailure, error.to_string()))
    }
}

/// Tracing config for Solidity stack trace generation.
#[napi(object)]
pub struct TracingConfigWithBuffers {
    /// Build information to use for decoding contracts. Either a Hardhat v2
    /// build info file that contains both input and output or a Hardhat v3
    /// build info file that doesn't contain output and a separate output file.
    pub build_infos: Option<Either<Vec<Uint8Array>, Vec<BuildInfoAndOutput>>>,
    /// Whether to ignore contracts whose name starts with "Ignored".
    pub ignore_contracts: Option<bool>,
}

impl std::fmt::Debug for TracingConfigWithBuffers {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let build_infos = self.build_infos.as_ref().map_or_else(
            || "None".to_string(),
            |bi| match bi {
                Either::A(arrays) => format!("Uint8Array[{}]", arrays.len()),
                Either::B(build_infos) => format!("BuildInfoAndOutput[{}]", build_infos.len()),
            },
        );
        f.debug_struct("TracingConfigWithBuffers")
            .field("build_infos", &build_infos)
            .field("ignore_contracts", &self.ignore_contracts)
            .finish()
    }
}

/// Hardhat V3 build info where the compiler output is not part of the build
/// info file.
#[napi(object)]
pub struct BuildInfoAndOutput {
    /// The build info input file
    pub build_info: Uint8Array,
    /// The build info output file
    pub output: Uint8Array,
}

impl<'a> From<&'a BuildInfoAndOutput>
    for edr_solidity::artifacts::BuildInfoBufferSeparateOutput<'a>
{
    fn from(value: &'a BuildInfoAndOutput) -> Self {
        Self {
            build_info: value.build_info.as_ref(),
            output: value.output.as_ref(),
        }
    }
}

impl<'a> From<&'a TracingConfigWithBuffers>
    for edr_solidity::artifacts::BuildInfoConfigWithBuffers<'a>
{
    fn from(value: &'a TracingConfigWithBuffers) -> Self {
        use edr_solidity::artifacts::{BuildInfoBufferSeparateOutput, BuildInfoBuffers};

        let build_infos = value.build_infos.as_ref().map(|infos| match infos {
            Either::A(with_output) => BuildInfoBuffers::WithOutput(
                with_output
                    .iter()
                    .map(std::convert::AsRef::as_ref)
                    .collect(),
            ),
            Either::B(separate_output) => BuildInfoBuffers::SeparateInputOutput(
                separate_output
                    .iter()
                    .map(BuildInfoBufferSeparateOutput::from)
                    .collect(),
            ),
        });

        Self {
            build_infos,
            ignore_contracts: value.ignore_contracts,
        }
    }
}

#[derive(Debug)]
struct SolidityTraceData {
    trace: Arc<edr_evm::trace::Trace>,
    contract_decoder: Arc<ContractDecoder>,
}

#[napi]
pub struct Response {
    // N-API is known to be slow when marshalling `serde_json::Value`s, so we try to return a
    // `String`. If the object is too large to be represented as a `String`, we return a `Buffer`
    // instead.
    data: Either<String, serde_json::Value>,
    /// When a transaction fails to execute, the provider returns a trace of the
    /// transaction.
    solidity_trace: Option<SolidityTraceData>,
    /// This may contain zero or more traces, depending on the (batch) request
    traces: Vec<Arc<edr_evm::trace::Trace>>,
}

#[napi]
impl Response {
    /// Returns the response data as a JSON string or a JSON object.
    #[napi(getter)]
    pub fn data(&self) -> Either<String, serde_json::Value> {
        self.data.clone()
    }

    #[napi(getter)]
    pub fn traces(&self) -> Vec<RawTrace> {
        self.traces
            .iter()
            .map(|trace| RawTrace::new(trace.clone()))
            .collect()
    }

    // Rust port of https://github.com/NomicFoundation/hardhat/blob/c20bf195a6efdc2d74e778b7a4a7799aac224841/packages/hardhat-core/src/internal/hardhat-network/provider/provider.ts#L590
    #[doc = "Compute the error stack trace. Return the stack trace if it can be decoded, otherwise returns none. Throws if there was an error computing the stack trace."]
    #[napi]
    pub async fn set_verbose_tracing(&self, verbose_tracing: bool) -> napi::Result<()> {
        let provider = self.provider.clone();

        if let Some(nested_trace) = nested_trace {
            let decoded_trace = contract_decoder
                .try_to_decode_nested_trace(nested_trace)
                .map_err(|err| napi::Error::from_reason(err.to_string()))?;
            let stack_trace = edr_solidity::solidity_tracer::get_stack_trace(decoded_trace)
                .map_err(|err| napi::Error::from_reason(err.to_string()))?;
            let stack_trace = stack_trace
                .into_iter()
                .map(super::cast::TryCast::try_cast)
                .collect::<Result<Vec<_>, _>>()?;

            Ok(Some(stack_trace))
        } else {
            Ok(None)
        }
    }
}
