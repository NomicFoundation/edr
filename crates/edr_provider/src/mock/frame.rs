use edr_evm::{
    evm::{Frame, FrameOrResultGen, FrameResult},
    interpreter::{CallInputs, CallOutcome, FrameInput, Gas, InstructionResult, InterpreterResult},
};

use super::{context::MockerMutGetter, CallOverrideResult};

pub struct MockingFrame<FrameT: Frame> {
    inner: FrameT,
}

impl<FrameT: Frame> MockingFrame<FrameT> {
    pub fn new(inner: FrameT) -> Self {
        Self { inner }
    }

    fn try_mocking_call<ContextT>(
        context: &mut ContextT,
        inputs: &CallInputs,
    ) -> Option<FrameOrResultGen<Self, FrameResult>>
    where
        ContextT: MockerMutGetter,
    {
        let mocker = context.mocker_mut();
        mocker
            .override_call(inputs.bytecode_address, inputs.input.clone())
            .map(
                |CallOverrideResult {
                     output,
                     should_revert,
                 }| {
                    let result = if should_revert {
                        InstructionResult::Revert
                    } else {
                        InstructionResult::Return
                    };

                    Some(FrameOrResultGen::Result(FrameResult::Call(
                        CallOutcome::new(
                            InterpreterResult {
                                result,
                                output,
                                gas: Gas::new(inputs.gas_limit),
                            },
                            inputs.return_memory_offset,
                        ),
                    )))
                },
            )
    }
}

impl<ContextT, FrameT> Frame for MockingFrame<FrameT>
where
    ContextT: MockerMutGetter,
    FrameT: for<'context> Frame<
        Context<'context> = ContextT,
        FrameInit = FrameInput,
        FrameResult = FrameResult,
    >,
{
    type Context<'context> = ContextT;
    type FrameInit = FrameT::FrameInit;
    type FrameResult = FrameT::FrameResult;
    type Error = FrameT::Error;

    fn init_first(
        context: &mut Self::Context<'_>,
        frame_input: Self::FrameInit,
    ) -> Result<FrameOrResultGen<Self, Self::FrameResult>, Self::Error> {
    }

    fn final_return(
        context: &mut Self::Context<'_>,
        result: &mut Self::FrameResult,
    ) -> Result<(), Self::Error> {
        FrameT::final_return(context, result)
    }

    fn init(
        &self,
        context: &mut Self::Context<'_>,
        frame_input: Self::FrameInit,
    ) -> Result<FrameOrResultGen<Self, Self::FrameResult>, Self::Error> {
        todo!()
    }

    fn run(
        &mut self,
        context: &mut Self::Context<'_>,
    ) -> Result<FrameOrResultGen<Self::FrameInit, Self::FrameResult>, Self::Error> {
        self.inner.run(context)
    }

    fn return_result(
        &mut self,
        context: &mut Self::Context<'_>,
        result: Self::FrameResult,
    ) -> Result<(), Self::Error> {
        self.inner.return_result(context, result)
    }
}

/// Registers the `Mocker`'s handles.
pub fn register_mocking_handles<EvmWiringT>(handler: &mut EvmHandler<'_, EvmWiringT>)
where
    EvmWiringT:
        EvmWiring<ExternalContext: GetContextData<Mocker>, Database: Database<Error: Debug>>,
{
    let old_handle = handler.execution.call.clone();
    handler.execution.call = Arc::new(
        move |ctx, inputs| -> Result<FrameOrResult, EVMErrorWiring<EvmWiringT>> {
            let mocker = ctx.external.get_context_data();
            if let Some(CallOverrideResult {
                output,
                should_revert,
            }) = mocker.override_call(inputs.bytecode_address, inputs.input.clone())
            {
                let result = if should_revert {
                    InstructionResult::Revert
                } else {
                    InstructionResult::Return
                };

                Ok(FrameOrResult::Result(FrameResult::Call(CallOutcome::new(
                    InterpreterResult {
                        result,
                        output,
                        gas: Gas::new(inputs.gas_limit),
                    },
                    inputs.return_memory_offset,
                ))))
            } else {
                old_handle(ctx, inputs)
            }
        },
    );
}
