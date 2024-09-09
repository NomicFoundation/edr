//! N-API bindings for the Rust port of `VMTracer` from Hardhat.

use napi::{
    bindgen_prelude::{Either3, Either4, Undefined},
    Either, Env, JsError,
};
use napi_derive::napi;

use crate::trace::{
    message_trace::{
        message_trace_to_napi, CallMessageTrace, CreateMessageTrace, PrecompileMessageTrace,
    },
    RawTrace,
};

/// N-API bindings for the Rust port of `VMTracer` from Hardhat.
#[napi]
pub struct VMTracer(edr_solidity::vm_tracer::VmTracer);

#[napi]
impl VMTracer {
    #[napi(constructor)]
    pub fn new() -> napi::Result<Self> {
        Ok(Self(edr_solidity::vm_tracer::VmTracer::new()))
    }

    /// Observes a trace, collecting information about the execution of the EVM.
    #[napi]
    pub fn observe(&mut self, trace: &RawTrace) {
        self.0.observe(&trace.inner);
    }

    // Explicitly return undefined as `Option<T>` by default returns `null` in JS
    // and the null/undefined checks we use in JS are strict
    #[napi]
    pub fn get_last_top_level_message_trace(
        &self,
        env: Env,
    ) -> napi::Result<
        Either4<PrecompileMessageTrace, CallMessageTrace, CreateMessageTrace, Undefined>,
    > {
        Ok(
            match self
                .0
                .get_last_top_level_message_trace_ref()
                .map(|x| {
                    x.try_borrow()
                        .expect("cannot be executed concurrently with `VMTracer::observe`")
                        .clone()
                })
                .map(|msg| message_trace_to_napi(msg, env))
                .transpose()?
            {
                Some(Either3::A(precompile)) => Either4::A(precompile),
                Some(Either3::B(call)) => Either4::B(call),
                Some(Either3::C(create)) => Either4::C(create),
                None => Either4::D(()),
            },
        )
    }

    // Explicitly return undefined as `Option<T>` by default returns `null` in JS
    // and the null/undefined checks we use in JS are strict
    #[napi]
    pub fn get_last_error(&self) -> Either<JsError, Undefined> {
        match self.0.get_last_error() {
            Some(err) => Either::A(napi::Error::from_reason(err).into()),
            None => Either::B(()),
        }
    }
}
