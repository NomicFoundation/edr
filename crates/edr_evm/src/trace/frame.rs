use std::marker::PhantomData;

use edr_eth::spec::HaltReasonTrait;
use revm::{context_interface::JournalGetter, handler::FrameResult, interpreter::FrameInput};
use revm_handler_interface::{Frame, FrameOrResultGen};

use super::context::TraceCollectorGetter;
use crate::{
    blockchain::BlockHash,
    state::{DatabaseComponents, State, WrapDatabaseRef},
};

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
    fn notify_frame_end<BlockchainT, ContextT, StateT>(context: &mut ContextT, result: &FrameResult)
    where
        BlockchainT: BlockHash<Error: std::error::Error>,
        ContextT: JournalGetter<Database = WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>>
            + TraceCollectorGetter<HaltReasonT>,
        StateT: State<Error: std::error::Error>,
    {
        let trace_collector = context.trace_collector();
        match result {
            FrameResult::Call(outcome) => {
                trace_collector.call_end(context.journal_ref(), outcome);
            }
            FrameResult::Create(outcome) => {
                trace_collector.create_end(context.journal_ref(), outcome);
            }
            // TODO: https://github.com/NomicFoundation/edr/issues/427
            FrameResult::EOFCreate(_outcome) => unreachable!("EDR doesn't support EOF yet."),
        }
    }

    /// Notifies the collector that a frame has started.
    fn notify_frame_start<
        BlockchainT: BlockHash<Error: std::error::Error>,
        ContextT: JournalGetter<Database = WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>>
            + TraceCollectorGetter<HaltReasonT>,
        StateT: State<Error: std::error::Error>,
    >(
        context: &mut ContextT,
        frame_input: &FrameInput,
    ) {
        let trace_collector = context.trace_collector();
        match frame_input {
            FrameInput::Call(inputs) => trace_collector.call(context.journal_ref(), inputs),
            FrameInput::Create(inputs) => trace_collector.create(context.journal_ref(), inputs),
            // TODO: https://github.com/NomicFoundation/edr/issues/427
            FrameInput::EOFCreate(_inputs) => unreachable!("EDR doesn't support EOF yet."),
        }
    }
}

impl<BlockchainT, ContextT, FrameT, HaltReasonT, StateT> Frame
    for TraceCollectorFrame<FrameT, HaltReasonT>
where
    BlockchainT: BlockHash<Error: std::error::Error>,
    ContextT: JournalGetter<Database = WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>>
        + TraceCollectorGetter<HaltReasonT>,
    FrameT: for<'context> Frame<
        Context<'context> = ContextT,
        FrameInit = FrameInput,
        FrameResult = FrameResult,
    >,
    HaltReasonT: HaltReasonTrait + 'static,
    StateT: State<Error: std::error::Error>,
{
    type Context<'context> = ContextT;

    type FrameInit = FrameT::FrameInit;

    type FrameResult = FrameT::FrameResult;

    type Error = FrameT::Error;

    fn init_first(
        context: &mut Self::Context<'_>,
        frame_input: Self::FrameInit,
    ) -> Result<FrameOrResultGen<Self, Self::FrameResult>, Self::Error> {
        Self::notify_frame_start(context, &frame_input);

        let result =
            FrameT::init_first(context, frame_input).map(|frame| frame.map_frame(Self::new));

        match &result {
            Ok(FrameOrResultGen::Result(result)) => Self::notify_frame_end(context, result),
            _ => (),
        }

        result
    }

    fn final_return(
        context: &mut Self::Context<'_>,
        result: &mut Self::FrameResult,
    ) -> Result<(), Self::Error> {
        Self::notify_frame_end(context, result);

        FrameT::final_return(context, result)
    }

    fn init(
        &self,
        context: &mut Self::Context<'_>,
        frame_input: Self::FrameInit,
    ) -> Result<FrameOrResultGen<Self, Self::FrameResult>, Self::Error> {
        Self::notify_frame_start(context, &frame_input);

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
        Self::notify_frame_end(context, &result);
        context.trace_collector().finish_trace();

        self.inner.return_result(context, result)
    }
}
