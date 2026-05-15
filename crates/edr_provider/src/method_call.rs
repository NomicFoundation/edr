/// A JSON-RPC method call, consisting of the method name and parameters.
#[derive(serde::Deserialize)]
pub struct RpcMethodCall {
    pub method: String,
    pub params: Option<serde_json::Value>,
}

impl RpcMethodCall {
    /// Constructs a new instance from the given method name and parameters.
    pub fn with_params<ParamsT: serde::Serialize>(
        method: &str,
        params: ParamsT,
    ) -> Result<Self, serde_json::Error> {
        let params = serde_json::to_value(params)?;

        Ok(Self {
            method: method.to_owned(),
            params: Some(params),
        })
    }

    /// Constructs a new instance from the given method name and no parameters.
    pub fn without_params(method: &str) -> Self {
        Self {
            method: method.to_owned(),
            params: None,
        }
    }
}
