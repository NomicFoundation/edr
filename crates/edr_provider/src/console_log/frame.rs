use edr_evm::evm::{
    interpreter::{CallInputs, FrameInput},
    Frame,
};

use super::{ConsoleLogCollectorMutGetter, CONSOLE_ADDRESS};

/// An EVM frame used to collect console logs.
pub struct ConsoleLogCollectorFrame<FrameT: Frame> {
    inner: FrameT,
}

impl<FrameT: Frame> ConsoleLogCollectorFrame<FrameT> {
    /// Creates a new instance.
    pub fn new(inner: FrameT) -> Self {
        Self { inner }
    }

    /// Notifies the collector that a call started.
    fn notify_call_start<ContextT>(context: &mut ContextT, inputs: &CallInputs)
    where
        ContextT: ConsoleLogCollectorMutGetter,
    {
        if inputs.bytecode_address == CONSOLE_ADDRESS {
            let collector = context.console_log_collector_mut();
            collector.record_console_log(inputs.input.clone());
        }
    }
}

impl<ContextT, FrameT> Frame for ConsoleLogCollectorFrame<FrameT>
where
    ContextT: ConsoleLogCollectorMutGetter,
    FrameT: for<'context> Frame<Context<'context> = ContextT, FrameInit = FrameInput>,
{
    type Context<'context> = ContextT;

    type FrameInit = FrameT::FrameInit;

    type FrameResult = FrameT::FrameResult;

    type Error = FrameT::Error;

    fn init_first(
        context: &mut Self::Context<'_>,
        frame_input: Self::FrameInit,
    ) -> Result<FrameOrResultGen<Self, Self::FrameResult>, Self::Error> {
        if let FrameInput::Call(inputs) = &frame_input {
            Self::notify_call_start(context, inputs);
        }

        FrameT::init_first(context, frame_input).map(|frame| frame.map_frame(Self::new))
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
        if let FrameInput::Call(inputs) = &frame_input {
            Self::notify_call_start(context, inputs);
        }

        self.inner
            .init(context, frame_input)
            .map(|frame| frame.map_frame(Self::new))
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
