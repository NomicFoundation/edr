use std::marker::PhantomData;

use edr_eth::spec::HaltReasonTrait;
use revm::{context_interface::JournalGetter, handler::FrameResult, interpreter::FrameInput};
use revm_handler_interface::{Frame, FrameOrResultGen};

use super::TraceCollectorContext;

pub struct TraceCollectorFrame<FrameT: Frame, HaltReasonT: HaltReasonTrait> {
    inner: FrameT,
    _phantom: PhantomData<HaltReasonT>,
}

impl<FrameT: Frame, HaltReasonT: HaltReasonTrait> TraceCollectorFrame<FrameT, HaltReasonT> {
    /// Creates a new instance.
    fn new(inner: FrameT) -> Self {
        Self {
            inner,
            _phantom: PhantomData,
        }
    }

    /// Notifies the collector that a frame has ended.
    fn notify_frame_end<ContextT: JournalGetter>(
        context: &mut TraceCollectorContext<ContextT, HaltReasonT>,
        result: &FrameResult,
    ) {
        match result {
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
            // TODO: https://github.com/NomicFoundation/edr/issues/427
            FrameResult::EOFCreate(_outcome) => unreachable!("EDR doesn't support EOF yet."),
        }
    }

    /// Notifies the collector that a frame has started.
    fn notify_frame_start<ContextT: JournalGetter>(
        context: &mut TraceCollectorContext<ContextT, HaltReasonT>,
        frame_input: &FrameInput,
    ) {
        match frame_input {
            FrameInput::Call(inputs) => context.collector.call(context.inner.journal_ref(), inputs),
            FrameInput::Create(inputs) => context
                .collector
                .create(context.inner.journal_ref(), inputs),
            // TODO: https://github.com/NomicFoundation/edr/issues/427
            FrameInput::EOFCreate(_inputs) => unreachable!("EDR doesn't support EOF yet."),
        }
    }
}

impl<FrameT, HaltReasonT> Frame for TraceCollectorFrame<FrameT, HaltReasonT>
where
    FrameT: Frame<Context: JournalGetter, FrameInit = FrameInput, FrameResult = FrameResult>,
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
        Self::notify_frame_start(context, &frame_input);

        let result = FrameT::init_first(&mut context.inner, frame_input)
            .map(|frame| frame.map_frame(Self::new));

        match &result {
            Ok(FrameOrResultGen::Result(result)) => Self::notify_frame_end(context, result),
            _ => (),
        }

        result
    }

    fn final_return(
        context: &mut Self::Context,
        result: &mut Self::FrameResult,
    ) -> Result<(), Self::Error> {
        Self::notify_frame_end(context, result);

        FrameT::final_return(&mut context.inner, result)
    }

    fn init(
        &self,
        context: &mut Self::Context,
        frame_input: Self::FrameInit,
    ) -> Result<FrameOrResultGen<Self, Self::FrameResult>, Self::Error> {
        Self::notify_frame_start(context, &frame_input);

        self.inner
            .init(&mut context.inner, frame_input)
            .map(|frame| frame.map_frame(Self::new))
    }

    fn run(
        &mut self,
        context: &mut Self::Context,
    ) -> Result<FrameOrResultGen<Self::FrameInit, Self::FrameResult>, Self::Error> {
        self.inner.run(&mut context.inner)
    }

    fn return_result(
        &mut self,
        context: &mut Self::Context,
        result: Self::FrameResult,
    ) -> Result<(), Self::Error> {
        Self::notify_frame_end(context, &result);
        context.collector.finish_trace();

        self.inner.return_result(&mut context.inner, result)
    }
}
