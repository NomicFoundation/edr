use napi::{
    bindgen_prelude::{Either3, Either4, Undefined},
    Either, JsError,
};
use napi_derive::napi;

use crate::{
    message_trace::{
        message_trace_to_napi, CallMessageTrace, CreateMessageTrace, PrecompileMessageTrace,
    },
    trace::RawTrace,
};

#[napi]
pub struct VMTracer(edr_solidity::vm_tracer::VMTracer);

#[napi]
impl VMTracer {
    #[napi(constructor)]
    pub fn new() -> napi::Result<Self> {
        Ok(Self(edr_solidity::vm_tracer::VMTracer::new()))
    }

    /// Observes a trace, collecting information about the execution of the EVM.
    #[napi]
    pub fn observe(&mut self, trace: &RawTrace) {
        for msg in &trace.inner.messages {
            match msg.clone() {
                edr_evm::trace::TraceMessage::Before(before) => {
                    self.0.add_before_message(before);
                }
                edr_evm::trace::TraceMessage::Step(step) => {
                    self.0.add_step(step);
                }
                edr_evm::trace::TraceMessage::After(after) => {
                    self.0.add_after_message(after.execution_result);
                }
            }
        }
    }

    // Explicitly return undefined as `Option<T>` by default returns `null` in JS
    // and the null/undefined checks we use there are strict
    #[napi]
    pub fn get_last_top_level_message_trace(
        &self,
    ) -> Either4<PrecompileMessageTrace, CreateMessageTrace, CallMessageTrace, Undefined> {
        match self
            .0
            .get_last_top_level_message_trace()
            .cloned()
            .map(message_trace_to_napi)
        {
            Some(Either3::A(precompile)) => Either4::A(precompile),
            Some(Either3::B(create)) => Either4::B(create),
            Some(Either3::C(call)) => Either4::C(call),
            None => Either4::D(()),
        }
    }

    // Explicitly return undefined as `Option<T>` by default returns `null` in JS
    // and the null/undefined checks we use there are strict
    #[napi]
    pub fn get_last_error(&self) -> Either<JsError, Undefined> {
        match self.0.get_last_error() {
            Some(err) => Either::A(napi::Error::from_reason(err).into()),
            None => Either::B(()),
        }
    }
}
