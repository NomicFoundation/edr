pub mod time;

use std::sync::Arc;

use edr_napi_core::provider::SyncProvider;
use edr_rpc_client::jsonrpc;

/// A mock provider that always returns the given mocked response.
pub struct MockProvider {
    mocked_response: Box<serde_json::value::RawValue>,
}

impl MockProvider {
    pub fn new(mocked_response: serde_json::Value) -> napi::Result<Self> {
        let mocked_response = serde_json::value::to_raw_value(&mocked_response)
            .map_err(|error| napi::Error::new(napi::Status::InvalidArg, error.to_string()))?;

        Ok(Self { mocked_response })
    }
}

impl SyncProvider for MockProvider {
    fn enqueue_request(
        &self,
        _request: String,
        on_response: Box<dyn FnOnce(napi::Result<edr_napi_core::spec::Response>) + Send>,
    ) {
        // Constructing the mocked response does not wait, so it happens on the
        // calling thread.
        let response = jsonrpc::ResponseData::Success {
            result: self.mocked_response.clone(),
        };
        let response = edr_napi_core::spec::marshal_response_data(response)
            .map(|data| edr_napi_core::spec::Response {
                data,
                stack_trace_result: None,
                call_trace_arenas: Vec::new(),
            })
            .map_err(|error| napi::Error::new(napi::Status::GenericFailure, error.to_string()));

        on_response(response);
    }

    fn set_call_override_callback(
        &self,
        _call_override_callback: Arc<dyn edr_provider::SyncCallOverride>,
    ) -> napi::Result<()> {
        Ok(())
    }

    fn set_verbose_tracing(&self, _enabled: bool) -> napi::Result<()> {
        Ok(())
    }
}
