pub mod time;

use std::sync::Arc;

use edr_napi_core::provider::SyncProvider;
use edr_rpc_client::jsonrpc;

/// A mock provider that always returns the given mocked response.
pub struct MockProvider {
    mocked_response: serde_json::Value,
}

impl MockProvider {
    pub fn new(mocked_response: serde_json::Value) -> Self {
        Self { mocked_response }
    }
}

impl SyncProvider for MockProvider {
    fn handle_request(&self, _request: String) -> napi::Result<edr_napi_core::spec::Response> {
        let response = jsonrpc::ResponseData::Success {
            result: self.mocked_response.clone(),
        };
        edr_napi_core::spec::marshal_response_data(response)
            .map(|data| edr_napi_core::spec::Response {
                data,
                stack_trace_result: None,
                call_trace_arenas: Vec::new(),
            })
            .map_err(|error| napi::Error::new(napi::Status::GenericFailure, error.to_string()))
    }

    fn set_call_override_callback(
        &self,
        _call_override_callback: Arc<dyn edr_provider::SyncCallOverride>,
    ) {
    }

    fn set_verbose_tracing(&self, _enabled: bool) {}
}
