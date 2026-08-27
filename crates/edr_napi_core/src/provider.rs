mod config;

use std::{str::FromStr as _, sync::Arc};

use edr_provider::{time::TimeSinceEpoch, InvalidRequestReason, SyncCallOverride};
use edr_rpc_client::jsonrpc;

pub use self::config::{Config, ConfigOption};
use crate::spec::{Response, SyncNapiSpec};

/// Trait for a synchronous N-API provider that can be used for dynamic trait
/// objects.
pub trait SyncProvider: Send + Sync {
    /// Enqueues a request for execution, invoking `on_response` — potentially
    /// from a different thread — with the response once it is available.
    ///
    /// Implementations should not execute the request on the calling thread,
    /// returning immediately without waiting for the request to be handled, as
    /// the caller may be the JS main thread.
    fn enqueue_request(
        &self,
        request: String,
        on_response: Box<dyn FnOnce(napi::Result<Response>) + Send>,
    );

    /// Sets the call override callback.
    fn set_call_override_callback(
        &self,
        call_override_callback: Arc<dyn SyncCallOverride>,
    ) -> napi::Result<()>;

    /// Set the verbose tracing flag to the provided value.
    fn set_verbose_tracing(&self, enabled: bool) -> napi::Result<()>;
}

impl<ChainSpecT: SyncNapiSpec<TimerT>, TimerT: Clone + TimeSinceEpoch> SyncProvider
    for edr_provider::Provider<ChainSpecT, TimerT>
{
    fn enqueue_request(
        &self,
        request: String,
        on_response: Box<dyn FnOnce(napi::Result<Response>) + Send>,
    ) {
        // Deserialization of the JSON request typically only takes a few microseconds,
        // so we allow it on the calling thread.
        let request = match serde_json::from_str(&request) {
            Ok(request) => request,
            Err(error) => {
                // NOTE: This blocks on a round trip to the provider's background thread when
                // logging is enabled, but we allow this as it's not on the hot path of request
                // handling.
                on_response(handle_failed_deserialization(self, request, &error));
                return;
            }
        };

        edr_provider::Provider::enqueue_request(
            self,
            request,
            Box::new(move |response| on_response(ChainSpecT::cast_response(response))),
        );
    }

    fn set_call_override_callback(
        &self,
        call_override_callback: Arc<dyn SyncCallOverride>,
    ) -> napi::Result<()> {
        edr_provider::Provider::set_call_override_callback(self, Some(call_override_callback))
            .map_err(|error| napi::Error::new(napi::Status::GenericFailure, error.to_string()))
    }

    fn set_verbose_tracing(&self, enabled: bool) -> napi::Result<()> {
        edr_provider::Provider::set_verbose_tracing(self, enabled)
            .map_err(|error| napi::Error::new(napi::Status::GenericFailure, error.to_string()))
    }
}

/// Constructs the JSON-RPC error response for a request that failed to
/// deserialize, logging the failure through the provider where relevant.
fn handle_failed_deserialization<ChainSpecT, TimerT>(
    provider: &edr_provider::Provider<ChainSpecT, TimerT>,
    request: String,
    error: &serde_json::Error,
) -> napi::Result<Response>
where
    ChainSpecT: SyncNapiSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
{
    let message = error.to_string();

    let request = serde_json::Value::from_str(&request).ok();
    let method_name = request
        .as_ref()
        .and_then(|request| request.get("method"))
        .and_then(serde_json::Value::as_str);

    let reason = InvalidRequestReason::new(method_name, &message);

    // HACK: We need to log failed deserialization attempts when they concern input
    // validation.
    if let Some((method_name, provider_error)) = reason.provider_error::<ChainSpecT, TimerT>() {
        // Ignore potential failure of logging, as returning the original error is more
        // important
        let _result = provider.log_failed_deserialization(method_name, provider_error);
    }

    let response = jsonrpc::ResponseData::<()>::Error {
        error: jsonrpc::Error {
            code: reason.error_code(),
            message: reason.error_message(),
            data: request,
        },
    };

    serde_json::to_string(&response)
        .map_err(|error| {
            napi::Error::new(
                napi::Status::Unknown,
                format!("Failed to serialize response due to: {error}"),
            )
        })
        .map(Response::from)
}
