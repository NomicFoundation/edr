use std::sync::Arc;

use edr_napi_core::provider::SyncProvider;
use edr_rpc_client::jsonrpc;
use edr_solidity::contract_decoder::ContractDecoder;

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
    fn handle_request(
        &self,
        _request: String,
        _contract_decoder: Arc<ContractDecoder>,
    ) -> napi::Result<edr_napi_core::spec::Response<edr_eth::l1::HaltReason>> {
        let response = jsonrpc::ResponseData::Success {
            result: self.mocked_response.clone(),
        };
        edr_napi_core::spec::marshal_response_data(response)
            .map(|data| edr_napi_core::spec::Response {
                solidity_trace: None,
                data,
                traces: Vec::new(),
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
