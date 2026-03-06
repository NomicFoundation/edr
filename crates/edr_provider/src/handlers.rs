use edr_primitives::HashMap;

use crate::handlers::error::DynProviderError;

pub mod error;
pub mod eth;

/// A JSON-RPC request to the provider.
pub enum RpcRequest {
    /// A single JSON-RPC request
    Single(RpcMethodCall),
    /// A batch of JSON-RPC requests
    Batch(Vec<RpcMethodCall>),
}

/// A JSON-RPC method call, consisting of the method name and parameters.
pub struct RpcMethodCall {
    method: String,
    params: serde_json::Value,
}

pub struct Test {
    handlers: HashMap<&'static str, fn(serde_json::Value) -> serde_json::Value>,
}

impl Test {
    pub fn handle_request(
        &self,
        request: RpcMethodCall,
    ) -> Result<serde_json::Value, DynProviderError> {
        if let Some(handler) = self.handlers.get(request.method.as_str()) {
            handler()
        } else {
            serde_json::json!({
                "error": format!("Method {} not found", request.method)
            })
        }
    }
}

fn wrapper<HandlerT: Fn(ParamsT) -> Result<SuccessT, DynProviderError>, ParamsT, SuccessT>(
    params: serde_json::Value,
) -> Result<serde_json::Value, DynProviderError> {
}
