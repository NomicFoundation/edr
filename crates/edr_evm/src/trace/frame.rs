use std::marker::PhantomData;

use edr_eth::spec::HaltReasonTrait;
use revm::{context_interface::JournalGetter, handler::FrameResult, interpreter::FrameInput};
use revm_handler_interface::{Frame, FrameOrResultGen};

use super::TraceCollectorContext;

pub struct TraceCollectorFrame<FrameT: Frame, HaltReasonT: HaltReasonTrait> {
    inner: FrameT,
    _phantom: PhantomData<HaltReasonT>,
}

impl<FrameT, HaltReasonT> Frame for TraceCollectorFrame<FrameT, HaltReasonT>
where
    FrameT: Frame<Context: JournalGetter, FrameInit = FrameInput>,
    HaltReasonT: HaltReasonTrait,
{
    type Context = TraceCollectorContext<FrameT::Context, HaltReasonT>;

    type FrameInit = FrameT::FrameInit;

    type FrameResult = FrameT::FrameResult;

    type Error = FrameT::Error;

    fn init_first(
        context: &mut Self::Context,
        frame_input: Self::FrameInit,
    ) -> Result<FrameOrResultGen<Self, Self::FrameResult>, Self::Error> {
        match &frame_input {
            FrameInput::Call(inputs) => context.collector.call(context.inner.journal_ref(), inputs),
            FrameInput::Create(inputs) => context
                .collector
                .create(context.inner.journal_ref(), inputs),
            // TODO: https://github.com/NomicFoundation/edr/issues/427
            FrameInput::EOFCreate(_inputs) => unreachable!("EDR doesn't support EOF yet."),
        }

        let result = FrameT::init_first(context.inner, frame_input).map(|frame| {
            frame.map_frame(|inner| Self {
                inner,
                _phantom: PhantomData,
            })
        });

        match &result {
            Ok(FrameOrResultGen::Result(result)) => match result {
                FrameResult::Call(outcome) => {
                    context
                        .collector
                        .call_end(context.inner.journal_ref(), outcome);
                }
                FrameResult::Create(outcome) => {
                    context
                        .collector
                        .create_end(context.inner.journal_ref(), outcome);
                }
                FrameResult::EOFCreate(outcome) => unreachable!("EDR doesn't support EOF yet."),
            },
            _ => (),
        }

        Ok(result)
    }

    fn final_return(
        context: &mut Self::Context,
        result: &mut Self::FrameResult,
    ) -> Result<(), Self::Error> {
        todo!()
    }

    fn init(
        &self,
        context: &mut Self::Context,
        frame_input: Self::FrameInit,
    ) -> Result<FrameOrResultGen<Self, Self::FrameResult>, Self::Error> {
        todo!()
    }

    fn run(
        &mut self,
        context: &mut Self::Context,
    ) -> Result<FrameOrResultGen<Self::FrameInit, Self::FrameResult>, Self::Error> {
        todo!()
    }

    fn return_result(
        &mut self,
        context: &mut Self::Context,
        result: Self::FrameResult,
    ) -> Result<(), Self::Error> {
        todo!()
    }
}
