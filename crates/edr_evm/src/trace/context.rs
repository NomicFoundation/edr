use core::marker::PhantomData;

use edr_eth::spec::HaltReasonTrait;
use revm::JournaledState;
use revm_interpreter::{interpreter::EthInterpreter, Interpreter};

use super::TraceCollector;
use crate::{
    blockchain::BlockHash,
    debug::ExtendedContext,
    instruction::InspectsInstructionWithJournal,
    state::{DatabaseComponents, State, WrapDatabaseRef},
};

/// Trait for retrieving a mutable reference to a [`TraceCollector`] instance.
pub trait TraceCollectorMutGetter<HaltReasonT: HaltReasonTrait> {
    /// Retrieves a mutable reference to a [`TraceCollector`] instance.
    fn trace_collector_mut(&mut self) -> &mut TraceCollector<HaltReasonT>;
}

impl<'tracer, BlockchainT, HaltReasonT: HaltReasonTrait, StateT>
    TraceCollectorMutGetter<HaltReasonT>
    for TraceCollectorContext<'tracer, BlockchainT, HaltReasonT, StateT>
{
    fn trace_collector_mut(&mut self) -> &mut TraceCollector<HaltReasonT> {
        self.collector
    }
}

impl<'context, HaltReasonT: HaltReasonTrait, InnerContextT, OuterContextT>
    TraceCollectorMutGetter<HaltReasonT> for ExtendedContext<'context, InnerContextT, OuterContextT>
where
    OuterContextT: TraceCollectorMutGetter<HaltReasonT>,
{
    fn trace_collector_mut(&mut self) -> &mut TraceCollector<HaltReasonT> {
        self.extension.trace_collector_mut()
    }
}

pub struct TraceCollectorContext<'tracer, BlockchainT, HaltReasonT: HaltReasonTrait, StateT> {
    collector: &'tracer mut TraceCollector<HaltReasonT>,
    phantom: PhantomData<(BlockchainT, StateT)>,
}

impl<'tracer, BlockchainT, HaltReasonT: HaltReasonTrait, StateT>
    TraceCollectorContext<'tracer, BlockchainT, HaltReasonT, StateT>
{
    /// Creates a new instance.
    pub fn new(collector: &'tracer mut TraceCollector<HaltReasonT>) -> Self {
        Self {
            collector,
            phantom: PhantomData,
        }
    }
}

impl<
        'tracer,
        BlockchainT: BlockHash<Error: std::error::Error>,
        HaltReasonT: HaltReasonTrait,
        StateT: State<Error: std::error::Error>,
    > InspectsInstructionWithJournal
    for TraceCollectorContext<'tracer, BlockchainT, HaltReasonT, StateT>
{
    // TODO: Make this chain-agnostic
    type InterpreterTypes = EthInterpreter;
    type Journal = JournaledState<WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>>;

    fn before_instruction_with_journal(
        &mut self,
        interpreter: &Interpreter<Self::InterpreterTypes>,
        journal: &Self::Journal,
    ) {
        self.collector.step(interpreter, journal);
    }

    fn after_instruction_with_journal(
        &mut self,
        _interpreter: &Interpreter<Self::InterpreterTypes>,
        _journal: &Self::Journal,
    ) {
    }
}
