use core::fmt::Debug;
use std::sync::Arc;

use dyn_clone::DynClone;
use edr_evm::{
    inspector::Inspector,
    interpreter::{
        CallInputs, CallOutcome, EthInterpreter, Gas, InstructionResult, InterpreterResult,
    },
    spec::ContextTrait,
};
use edr_primitives::{Address, Bytes};

/// The result of executing a call override.
#[derive(Debug)]
pub struct CallOverrideResult {
    pub output: Bytes,
    pub should_revert: bool,
}

pub trait SyncCallOverride:
    Fn(Address, Bytes) -> Option<CallOverrideResult> + DynClone + Send + Sync
{
}

impl<F> SyncCallOverride for F where
    F: Fn(Address, Bytes) -> Option<CallOverrideResult> + DynClone + Send + Sync
{
}

dyn_clone::clone_trait_object!(SyncCallOverride);

pub struct Mocker {
    call_override: Option<Arc<dyn SyncCallOverride>>,
}

impl Mocker {
    /// Constructs a new instance with the provided call override.
    pub fn new(call_override: Option<Arc<dyn SyncCallOverride>>) -> Self {
        Self { call_override }
    }

    fn override_call(&self, contract: Address, input: Bytes) -> Option<CallOverrideResult> {
        self.call_override.as_ref().and_then(|f| f(contract, input))
    }

    fn try_mocking_call(
        &mut self,
        context: &mut impl ContextTrait,
        inputs: &mut CallInputs,
    ) -> Option<CallOutcome> {
        let input_data = inputs.input.bytes(context);
        self.override_call(inputs.bytecode_address, input_data).map(
            |CallOverrideResult {
                 output,
                 should_revert,
             }| {
                let result = if should_revert {
                    InstructionResult::Revert
                } else {
                    InstructionResult::Return
                };

                CallOutcome::new(
                    InterpreterResult {
                        result,
                        output,
                        gas: Gas::new(inputs.gas_limit),
                    },
                    inputs.return_memory_offset.clone(),
                )
            },
        )
    }
}

impl<ContextT: ContextTrait> Inspector<ContextT, EthInterpreter> for Mocker {
    fn call(&mut self, context: &mut ContextT, inputs: &mut CallInputs) -> Option<CallOutcome> {
        self.try_mocking_call(context, inputs)
    }
}
