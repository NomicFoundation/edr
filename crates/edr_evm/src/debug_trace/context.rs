use std::marker::PhantomData;

use edr_eth::spec::HaltReasonTrait;
use revm::JournaledState;
use revm_interpreter::{interpreter::EthInterpreter, Interpreter};

use super::TracerEip3155;
use crate::{
    blockchain::BlockHash,
    debug::ExtendedContext,
    instruction::InspectsInstructionWithJournal,
    state::{DatabaseComponents, State, WrapDatabaseRef},
    trace::{TraceCollector, TraceCollectorContext, TraceCollectorMutGetter},
};

/// Trait for retrieving a mutable reference to a [`TracerEip3155`] instance.
pub trait Eip3155TracerMutGetter {
    /// Retrieves a mutable reference to a [`TracerEip3155`] instance.
    fn eip3155_tracer_mut(&mut self) -> &mut TracerEip3155;
}

impl<'tracer, BlockchainT, StateT> Eip3155TracerMutGetter
    for Eip3155TracerContext<'tracer, BlockchainT, StateT>
{
    fn eip3155_tracer_mut(&mut self) -> &mut TracerEip3155 {
        self.tracer
    }
}

impl<'context, InnerContextT, OuterContextT> Eip3155TracerMutGetter
    for ExtendedContext<'context, InnerContextT, OuterContextT>
where
    OuterContextT: Eip3155TracerMutGetter,
{
    fn eip3155_tracer_mut(&mut self) -> &mut TracerEip3155 {
        self.extension.eip3155_tracer_mut()
    }
}

/// An EVM context that can be used to create EIP-3155 traces.
pub struct Eip3155TracerContext<'tracer, BlockchainT, StateT> {
    phantom: PhantomData<(BlockchainT, StateT)>,
    tracer: &'tracer mut TracerEip3155,
}

impl<'tracer, BlockchainT, StateT> Eip3155TracerContext<'tracer, BlockchainT, StateT> {
    /// Creates a new instance.
    pub fn new(tracer: &'tracer mut TracerEip3155) -> Self {
        Self {
            phantom: PhantomData,
            tracer,
        }
    }
}

impl<
        'tracer,
        BlockchainT: BlockHash<Error: std::error::Error>,
        StateT: State<Error: std::error::Error>,
    > InspectsInstructionWithJournal for Eip3155TracerContext<'tracer, BlockchainT, StateT>
{
    // TODO: Make this chain-agnostic
    type InterpreterTypes = EthInterpreter;
    type Journal = JournaledState<WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>>;

    fn before_instruction_with_journal(
        &mut self,
        interpreter: &Interpreter<Self::InterpreterTypes>,
        _journal: &Self::Journal,
    ) {
        self.tracer.step(interpreter);
    }

    fn after_instruction_with_journal(
        &mut self,
        interpreter: &Interpreter<Self::InterpreterTypes>,
        journal: &Self::Journal,
    ) {
        self.tracer.step_end(interpreter, journal);
    }
}

/// EIP-3155 and raw tracers.
pub struct Eip3155AndRawTracersContext<'tracer, BlockchainT, HaltReasonT: HaltReasonTrait, StateT> {
    eip3155: Eip3155TracerContext<'tracer, BlockchainT, StateT>,
    raw: TraceCollectorContext<'tracer, BlockchainT, HaltReasonT, StateT>,
}

impl<'tracer, BlockchainT, HaltReasonT: HaltReasonTrait, StateT>
    Eip3155AndRawTracersContext<'tracer, BlockchainT, HaltReasonT, StateT>
{
    /// Creates a new instance.
    pub fn new(
        eip3155: &'tracer mut TracerEip3155,
        raw: &'tracer mut TraceCollector<HaltReasonT>,
    ) -> Self {
        Self {
            eip3155: Eip3155TracerContext::new(eip3155),
            raw: TraceCollectorContext::new(raw),
        }
    }
}

impl<'tracer, BlockchainT, HaltReasonT, StateT> Eip3155TracerMutGetter
    for Eip3155AndRawTracersContext<'tracer, BlockchainT, HaltReasonT, StateT>
where
    HaltReasonT: HaltReasonTrait,
{
    fn eip3155_tracer_mut(&mut self) -> &mut TracerEip3155 {
        self.eip3155.eip3155_tracer_mut()
    }
}

impl<'tracer, BlockchainT, HaltReasonT, StateT> InspectsInstructionWithJournal
    for Eip3155AndRawTracersContext<'tracer, BlockchainT, HaltReasonT, StateT>
where
    BlockchainT: BlockHash<Error: std::error::Error>,
    HaltReasonT: HaltReasonTrait,
    StateT: State<Error: std::error::Error>,
{
    // TODO: Make this chain-agnostic
    type InterpreterTypes = EthInterpreter;
    type Journal = JournaledState<WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>>;

    fn before_instruction_with_journal(
        &mut self,
        interpreter: &Interpreter<Self::InterpreterTypes>,
        journal: &Self::Journal,
    ) {
        self.eip3155
            .before_instruction_with_journal(interpreter, journal);
        self.raw
            .before_instruction_with_journal(interpreter, journal);
    }

    fn after_instruction_with_journal(
        &mut self,
        interpreter: &Interpreter<Self::InterpreterTypes>,
        journal: &Self::Journal,
    ) {
        self.eip3155
            .after_instruction_with_journal(interpreter, journal);
        self.raw
            .after_instruction_with_journal(interpreter, journal);
    }
}

impl<'tracer, BlockchainT, HaltReasonT, StateT> TraceCollectorMutGetter<HaltReasonT>
    for Eip3155AndRawTracersContext<'tracer, BlockchainT, HaltReasonT, StateT>
where
    HaltReasonT: HaltReasonTrait,
{
    fn trace_collector_mut(&mut self) -> &mut TraceCollector<HaltReasonT> {
        self.raw.trace_collector_mut()
    }
}
