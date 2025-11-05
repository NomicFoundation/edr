use edr_chain_spec::EvmHaltReason;
use edr_napi_core::spec::SolidityTraceData;
use edr_solidity::contract_decoder::NestedTraceDecoder as _;
use napi::Either;
use napi_derive::napi;

use crate::{
    cast::TryCast,
    trace::{solidity_stack_trace::SolidityStackTrace, RawTrace},
};

#[napi]
pub struct Response {
    inner: edr_napi_core::spec::Response<EvmHaltReason>,
}

impl From<edr_napi_core::spec::Response<EvmHaltReason>> for Response {
    fn from(value: edr_napi_core::spec::Response<EvmHaltReason>) -> Self {
        Self { inner: value }
    }
}

#[napi]
impl Response {
    #[doc = "Returns the response data as a JSON string or a JSON object."]
    #[napi(catch_unwind, getter)]
    pub fn data(&self) -> Either<String, serde_json::Value> {
        self.inner.data.clone()
    }

    // Rust port of https://github.com/NomicFoundation/hardhat/blob/c20bf195a6efdc2d74e778b7a4a7799aac224841/packages/hardhat-core/src/internal/hardhat-network/provider/provider.ts#L590
    #[doc = "Compute the error stack trace. Return the stack trace if it can be decoded, otherwise returns none. Throws if there was an error computing the stack trace."]
    #[napi(catch_unwind)]
    pub fn stack_trace(&self) -> napi::Result<Option<SolidityStackTrace>> {
        let Some(SolidityTraceData {
            trace,
            contract_decoder,
        }) = &self.inner.solidity_trace
        else {
            return Ok(None);
        };
        let nested_trace = edr_solidity::nested_tracer::convert_trace_messages_to_nested_trace(
            trace.as_ref().clone(),
        )
        .map_err(|err| napi::Error::from_reason(err.to_string()))?;

        if let Some(vm_trace) = nested_trace {
            let decoded_trace = contract_decoder
                .try_to_decode_nested_trace(vm_trace)
                .map_err(|err| napi::Error::from_reason(err.to_string()))?;
            let stack_trace = edr_solidity::solidity_tracer::get_stack_trace(decoded_trace)
                .map_err(|err| napi::Error::from_reason(err.to_string()))?;
            let stack_trace = stack_trace
                .into_iter()
                .map(TryCast::try_cast)
                .collect::<Result<Vec<_>, _>>()?;

            Ok(Some(stack_trace))
        } else {
            Ok(None)
        }
    }

    #[doc = "Returns the raw traces of executed contracts. This maybe contain zero or more traces."]
    #[napi(catch_unwind, getter)]
    pub fn traces(&self) -> Vec<RawTrace> {
        self.inner
            .traces
            .iter()
            .map(|trace| RawTrace::from(trace.clone()))
            .collect()
    }
}
