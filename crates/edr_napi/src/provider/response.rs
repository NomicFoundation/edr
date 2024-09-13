use edr_generic::GenericChainSpec;
use napi::Either;
use napi_derive::napi;

use crate::trace::RawTrace;

#[napi]
pub struct Response {
    inner: edr_napi_core::spec::Response<GenericChainSpec>,
}

impl From<edr_napi_core::spec::Response<GenericChainSpec>> for Response {
    fn from(value: edr_napi_core::spec::Response<GenericChainSpec>) -> Self {
        Self { inner: value }
    }
}

#[napi]
impl Response {
    #[doc = "Returns the response data as a JSON string or a JSON object."]
    #[napi(getter)]
    pub fn data(&self) -> Either<String, serde_json::Value> {
        self.inner.data.clone()
    }

    #[doc = "Returns the Solidity trace of the transaction that failed to execute, if any."]
    #[napi(getter)]
    pub fn solidity_trace(&self) -> Option<RawTrace> {
        self.inner
            .solidity_trace
            .as_ref()
            .map(|trace| RawTrace::from(trace.clone()))
    }

    #[doc = "Returns the raw traces of executed contracts. This maybe contain zero or more traces."]
    #[napi(getter)]
    pub fn traces(&self) -> Vec<RawTrace> {
        self.inner
            .traces
            .iter()
            .map(|trace| RawTrace::from(trace.clone()))
            .collect()
    }
}
