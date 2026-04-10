mod config;
mod factory;

use std::sync::Arc;

use edr_provider::{time::TimeSinceEpoch, SyncCallOverride};
use edr_rpc_client::jsonrpc;

pub use self::{config::Config, factory::SyncProviderFactory};
use crate::spec::{Response, SyncNapiSpec};

/// Trait for a synchronous N-API provider that can be used for dynamic trait
/// objects.
pub trait SyncProvider: Send + Sync {
    /// Blocking method to handle a request.
    fn handle_request(&self, request: String) -> napi::Result<Response>;

    /// Set to `true` to make the traces returned with `eth_call`,
    /// `eth_estimateGas`, `eth_sendRawTransaction`, `eth_sendTransaction`,
    /// `evm_mine`, `hardhat_mine` include the full stack and memory. Set to
    /// `false` to disable this.
    fn set_call_override_callback(&self, call_override_callback: Arc<dyn SyncCallOverride>);

    /// Set the verbose tracing flag to the provided value.
    fn set_verbose_tracing(&self, enabled: bool);
}

impl<ChainSpecT: SyncNapiSpec<TimerT>, TimerT: Clone + TimeSinceEpoch> SyncProvider
    for edr_provider::Provider<ChainSpecT, TimerT>
{
    fn handle_request(&self, request: String) -> napi::Result<Response> {
        let Ok(request) = serde_json::from_str(&request) else {
            let response = jsonrpc::ResponseData::<()>::Error {
                error: jsonrpc::Error {
                    code: -32600, // Invalid Request
                    message: "Invalid Request".to_string(),
                    data: None,
                },
            };

            return serde_json::to_string(&response)
                .map_err(|error| {
                    napi::Error::new(
                        napi::Status::Unknown,
                        format!("Failed to serialize response due to: {error}"),
                    )
                })
                .map(Response::from);
        };

        let response = edr_provider::Provider::handle_request(self, request);

        ChainSpecT::cast_response(response)
    }

    fn set_call_override_callback(&self, call_override_callback: Arc<dyn SyncCallOverride>) {
        self.set_call_override_callback(Some(call_override_callback));
    }

    fn set_verbose_tracing(&self, enabled: bool) {
        self.set_verbose_tracing(enabled);
    }
}
