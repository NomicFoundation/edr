use napi::{
    bindgen_prelude::{Either4, Undefined},
    Either,
};
use napi_derive::napi;

use super::message_trace::{CallMessageTrace, CreateMessageTrace, EvmStep, PrecompileMessageTrace};

#[napi]
pub struct SolidityTracer;

#[napi]
impl SolidityTracer {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self
    }

    #[napi]
    pub fn _get_last_subtrace(
        &self,
        trace: Either<CallMessageTrace, CreateMessageTrace>,
    ) -> Either4<PrecompileMessageTrace, CallMessageTrace, CreateMessageTrace, Undefined> {
        let (number_of_subtraces, steps) = match trace {
            Either::A(create) => (create.number_of_subtraces, create.steps),
            Either::B(call) => (call.number_of_subtraces, call.steps),
        };

        if number_of_subtraces == 0 {
            return Either4::D(());
        }

        steps
            .into_iter()
            .rev()
            .find_map(|step| match step {
                Either4::A(EvmStep { .. }) => None,
                Either4::B(precompile) => Some(Either4::A(precompile)),
                Either4::C(call) => Some(Either4::B(call)),
                Either4::D(create) => Some(Either4::C(create)),
            })
            .unwrap_or(Either4::D(()))
    }
}
