use std::marker::PhantomData;

use edr_eth::spec::HaltReasonTrait;
use revm::{context_interface::JournalGetter, handler::FrameResult, interpreter::FrameInput};
use revm_handler_interface::{Frame, FrameOrResultGen};

use super::context::Eip3155TracerContext;

pub struct Eip3155TracerFrame<FrameT: Frame, HaltReasonT: HaltReasonTrait> {
    inner: FrameT,
    _phantom: PhantomData<HaltReasonT>,
}

impl<FrameT: Frame, HaltReasonT: HaltReasonTrait> Eip3155TracerFrame<FrameT, HaltReasonT> {
    /// Creates a new instance.
    fn new(inner: FrameT) -> Self {
        Self {
            inner,
            _phantom: PhantomData,
        }
    }

    /// Notifies the tracer that a frame has ended.
    fn notify_frame_end<ContextT: JournalGetter>(
        context: &mut Eip3155TracerContext<ContextT>,
        result: &FrameResult,
    ) {
        match result {
            FrameResult::Call(outcome) => {
                context.tracer.on_inner_frame_result(&outcome.result);
            }
            FrameResult::Create(outcome) => {
                context.tracer.on_inner_frame_result(&outcome.result);
            }
            // TODO: https://github.com/NomicFoundation/edr/issues/427
            FrameResult::EOFCreate(outcome) => unreachable!("EDR doesn't support EOF yet."),
        }
    }
}

impl<FrameT, HaltReasonT> Frame for Eip3155TracerFrame<FrameT, HaltReasonT>
where
    FrameT: Frame<Context: JournalGetter, FrameInit = FrameInput>,
    HaltReasonT: HaltReasonTrait,
{
    type Context = Eip3155TracerContext<FrameT::Context>;

    type FrameInit = FrameT::FrameInit;

    type FrameResult = FrameT::FrameResult;

    type Error = FrameT::Error;

    fn init_first(
        context: &mut Self::Context,
        frame_input: Self::FrameInit,
    ) -> Result<FrameOrResultGen<Self, Self::FrameResult>, Self::Error> {
        FrameT::init_first(context.inner, frame_input).map(|frame| frame.map_frame(Self::new))
    }

    fn final_return(
        context: &mut Self::Context,
        result: &mut Self::FrameResult,
    ) -> Result<(), Self::Error> {
        FrameT::final_return(context.inner, result)
    }

    fn init(
        &self,
        context: &mut Self::Context,
        frame_input: Self::FrameInit,
    ) -> Result<FrameOrResultGen<Self, Self::FrameResult>, Self::Error> {
        self.inner
            .init(context.inner, frame_input)
            .map(|frame| frame.map_frame(Self::new))
    }

    fn run(
        &mut self,
        context: &mut Self::Context,
    ) -> Result<FrameOrResultGen<Self::FrameInit, Self::FrameResult>, Self::Error> {
        self.inner.run(context.inner)
    }

    fn return_result(
        &mut self,
        context: &mut Self::Context,
        result: Self::FrameResult,
    ) -> Result<(), Self::Error> {
        Self::notify_frame_end(context, &result);

        self.inner.return_result(context, result)
    }
}
