use core::marker::PhantomData;

use edr_eth::{spec::HaltReasonTrait, Address, HashMap};
use revm::JournaledState;
use revm_interpreter::{interpreter::EthInterpreter, Interpreter};

use super::TraceCollector;
use crate::{
    blockchain::BlockHash,
    extension::ExtendedContext,
    instruction::InspectsInstructionWithJournal,
    precompile::{CustomPrecompilesGetter, PrecompileFn},
    state::{DatabaseComponents, State, WrapDatabaseRef},
};

/// Trait for retrieving a mutable reference to a [`TraceCollector`] instance.
pub trait TraceCollectorMutGetter<HaltReasonT: HaltReasonTrait> {
    /// Retrieves a mutable reference to a [`TraceCollector`] instance.
    fn trace_collector_mut(&mut self) -> &mut TraceCollector<HaltReasonT>;
}

impl<HaltReasonT: HaltReasonTrait, InnerContextT, OuterContextT>
    TraceCollectorMutGetter<HaltReasonT> for ExtendedContext<'_, InnerContextT, OuterContextT>
where
    OuterContextT: TraceCollectorMutGetter<HaltReasonT>,
{
    fn trace_collector_mut(&mut self) -> &mut TraceCollector<HaltReasonT> {
        self.extension.trace_collector_mut()
    }
}

/// An EVM context that can be used to collect raw traces.
pub struct TraceCollectorContext<'context, BlockchainT, HaltReasonT: HaltReasonTrait, StateT> {
    collector: &'context mut TraceCollector<HaltReasonT>,
    phantom: PhantomData<(BlockchainT, StateT)>,
}

impl<'context, BlockchainT, HaltReasonT: HaltReasonTrait, StateT>
    TraceCollectorContext<'context, BlockchainT, HaltReasonT, StateT>
{
    /// Creates a new instance.
    pub fn new(collector: &'context mut TraceCollector<HaltReasonT>) -> Self {
        Self {
            collector,
            phantom: PhantomData,
        }
    }
}

impl<
        BlockchainT: BlockHash<Error: std::error::Error>,
        HaltReasonT: HaltReasonTrait,
        StateT: State<Error: std::error::Error>,
    > InspectsInstructionWithJournal
    for TraceCollectorContext<'_, BlockchainT, HaltReasonT, StateT>
{
    // TODO: Make this chain-agnostic
    type InterpreterTypes = EthInterpreter;
    type Journal = JournaledState<WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>>;

    fn before_instruction_with_journal(
        &mut self,
        interpreter: &Interpreter<Self::InterpreterTypes>,
        journal: &Self::Journal,
    ) {
        self.collector.notify_step_start(interpreter, journal);
    }

    fn after_instruction_with_journal(
        &mut self,
        _interpreter: &Interpreter<Self::InterpreterTypes>,
        _journal: &Self::Journal,
    ) {
    }
}

impl<BlockchainT, HaltReasonT: HaltReasonTrait, StateT> TraceCollectorMutGetter<HaltReasonT>
    for TraceCollectorContext<'_, BlockchainT, HaltReasonT, StateT>
{
    fn trace_collector_mut(&mut self) -> &mut TraceCollector<HaltReasonT> {
        self.collector
    }
}

/// An EVM context that can be used to collect raw traces.
pub struct TraceCollectorContextWithPrecompiles<
    'context,
    BlockchainT,
    HaltReasonT: HaltReasonTrait,
    StateT,
> {
    collector: &'context mut TraceCollector<HaltReasonT>,
    custom_precompiles: &'context HashMap<Address, PrecompileFn>,
    phantom: PhantomData<(BlockchainT, StateT)>,
}

impl<'context, BlockchainT, HaltReasonT: HaltReasonTrait, StateT>
    TraceCollectorContextWithPrecompiles<'context, BlockchainT, HaltReasonT, StateT>
{
    /// Creates a new instance.
    pub fn new(
        collector: &'context mut TraceCollector<HaltReasonT>,
        custom_precompiles: &'context HashMap<Address, PrecompileFn>,
    ) -> Self {
        Self {
            collector,
            custom_precompiles,
            phantom: PhantomData,
        }
    }
}

impl<
        BlockchainT: BlockHash<Error: std::error::Error>,
        HaltReasonT: HaltReasonTrait,
        StateT: State<Error: std::error::Error>,
    > CustomPrecompilesGetter
    for TraceCollectorContextWithPrecompiles<'_, BlockchainT, HaltReasonT, StateT>
{
    fn custom_precompiles(&self) -> edr_eth::HashMap<Address, PrecompileFn> {
        self.custom_precompiles.clone()
    }
}

impl<
        BlockchainT: BlockHash<Error: std::error::Error>,
        HaltReasonT: HaltReasonTrait,
        StateT: State<Error: std::error::Error>,
    > InspectsInstructionWithJournal
    for TraceCollectorContextWithPrecompiles<'_, BlockchainT, HaltReasonT, StateT>
{
    // TODO: Make this chain-agnostic
    type InterpreterTypes = EthInterpreter;
    type Journal = JournaledState<WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>>;

    fn before_instruction_with_journal(
        &mut self,
        interpreter: &Interpreter<Self::InterpreterTypes>,
        journal: &Self::Journal,
    ) {
        self.collector.notify_step_start(interpreter, journal);
    }

    fn after_instruction_with_journal(
        &mut self,
        _interpreter: &Interpreter<Self::InterpreterTypes>,
        _journal: &Self::Journal,
    ) {
    }
}

impl<BlockchainT, HaltReasonT: HaltReasonTrait, StateT> TraceCollectorMutGetter<HaltReasonT>
    for TraceCollectorContextWithPrecompiles<'_, BlockchainT, HaltReasonT, StateT>
{
    fn trace_collector_mut(&mut self) -> &mut TraceCollector<HaltReasonT> {
        self.collector
    }
}
