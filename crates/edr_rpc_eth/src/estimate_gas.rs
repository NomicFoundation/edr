use crate::CallRequest;

/// For specifying input to `eth_estimateGas`.
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
#[repr(transparent)]
pub struct EstimateGasRequest {
    #[serde(flatten)]
    pub inner: CallRequest,
}

impl From<CallRequest> for EstimateGasRequest {
    fn from(inner: CallRequest) -> Self {
        Self { inner }
    }
}
