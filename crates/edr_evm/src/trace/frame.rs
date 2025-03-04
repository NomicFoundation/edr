use std::marker::PhantomData;

use edr_eth::spec::{ChainSpec, HaltReasonTrait};
use revm::{handler::FrameResult, interpreter::FrameInput};
use revm_context_interface::Journal;
use revm_handler_interface::{Frame, FrameOrResultGen};

use super::{context::TraceCollectorMutGetter, TraceCollector};
use crate::{
    blockchain::BlockHash,
    evm::EvmSpec,
    extension::ExtendedContext,
    instruction::InspectableInstructionProvider,
    interpreter::EthInterpreter,
    spec::RuntimeSpec,
    state::{DatabaseComponents, State, WrapDatabaseRef},
};

/// Mutable references to the journal and trace collector.
pub struct JournalAndTraceCollector<'getter, JournalT, HaltReasonT: HaltReasonTrait> {
    pub journal: &'getter mut JournalT,
    pub trace_collector: &'getter mut TraceCollector<HaltReasonT>,
}

pub trait JournalAndTraceCollectorGetter<HaltReasonT: HaltReasonTrait> {
    type Journal: Journal;

    /// Retrieves mutable references to the journal and trace collector.
    fn journal_and_trace_collector_mut(
        &mut self,
    ) -> JournalAndTraceCollector<'_, Self::Journal, HaltReasonT>;
}

impl<HaltReasonT: HaltReasonTrait, InnerContextT, OuterContextT>
    JournalAndTraceCollectorGetter<HaltReasonT>
    for ExtendedContext<'_, InnerContextT, OuterContextT>
where
    InnerContextT: JournalGetter,
    OuterContextT: TraceCollectorMutGetter<HaltReasonT>,
{
    type Journal = InnerContextT::Journal;

    fn journal_and_trace_collector_mut(
        &mut self,
    ) -> JournalAndTraceCollector<'_, Self::Journal, HaltReasonT> {
        JournalAndTraceCollector {
            journal: self.inner.journal(),
            trace_collector: self.extension.trace_collector_mut(),
        }
    }
}

/// An EVM frame used to collect raw traces.
pub struct RawTracerFrame<FrameT: Frame, HaltReasonT: HaltReasonTrait> {
    inner: FrameT,
    _phantom: PhantomData<HaltReasonT>,
}

impl<FrameT: Frame, HaltReasonT: HaltReasonTrait> RawTracerFrame<FrameT, HaltReasonT> {
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
        ContextT: JournalAndTraceCollectorGetter<
            HaltReasonT,
            Journal: Journal<Database = WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>>,
        >,
        StateT: State<Error: std::error::Error>,
    {
        let JournalAndTraceCollector {
            journal,
            trace_collector,
        } = context.journal_and_trace_collector_mut();

        match result {
            FrameResult::Call(outcome) => {
                trace_collector.notify_call_end(journal, outcome);
            }
            FrameResult::Create(outcome) => {
                trace_collector.notify_create_end(journal, outcome);
            }
            // TODO: https://github.com/NomicFoundation/edr/issues/427
            FrameResult::EOFCreate(_outcome) => unreachable!("EDR doesn't support EOF yet."),
        }
    }

    /// Notifies the collector that a frame has started.
    fn notify_frame_start<
        BlockchainT: BlockHash<Error: std::error::Error>,
        ContextT: JournalAndTraceCollectorGetter<
            HaltReasonT,
            Journal: Journal<Database = WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>>,
        >,
        StateT: State<Error: std::error::Error>,
    >(
        context: &mut ContextT,
        frame_input: &FrameInput,
    ) {
        let JournalAndTraceCollector {
            journal,
            trace_collector,
        } = context.journal_and_trace_collector_mut();

        match frame_input {
            FrameInput::Call(inputs) => trace_collector.notify_call_start(journal, inputs),
            FrameInput::Create(inputs) => trace_collector.notify_create_start(journal, inputs),
            // TODO: https://github.com/NomicFoundation/edr/issues/427
            FrameInput::EOFCreate(_inputs) => unreachable!("EDR doesn't support EOF yet."),
        }
    }
}

impl<BlockchainT, ContextT, FrameT, HaltReasonT, StateT> Frame
    for RawTracerFrame<FrameT, HaltReasonT>
where
    BlockchainT: BlockHash<Error: std::error::Error>,
    ContextT: JournalAndTraceCollectorGetter<
            HaltReasonT,
            Journal: Journal<Database = WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>>,
        > + TraceCollectorMutGetter<HaltReasonT>,
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

        if let Ok(FrameOrResultGen::Result(result)) = &result {
            Self::notify_frame_end(context, result);
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
        context.trace_collector_mut().finish_trace();

        self.inner.return_result(context, result)
    }
}

/// Helper type for a frame that collects raw traces for the provided precompile
/// provider type.
pub type RawTracerFrameWithPrecompileProvider<BlockchainErrorT, ChainSpecT, ContextT, PrecompileProviderT, StateErrorT> =
    RawTracerFrame<
        <<ChainSpecT as RuntimeSpec>::Evm<BlockchainErrorT, ContextT, StateErrorT> as EvmSpec<BlockchainErrorT, ChainSpecT, ContextT, StateErrorT>>::Frame<
            InspectableInstructionProvider<
                ContextT,
                EthInterpreter,
                <<ChainSpecT as RuntimeSpec>::Evm<BlockchainErrorT, ContextT, StateErrorT> as EvmSpec<BlockchainErrorT, ChainSpecT, ContextT, StateErrorT>>::InstructionProvider,
            >,
            PrecompileProviderT,
        >,
        <ChainSpecT as ChainSpec>::HaltReason
    >;
