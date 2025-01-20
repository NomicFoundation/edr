use std::marker::PhantomData;

use revm::JournaledState;
use revm_interpreter::{interpreter::EthInterpreter, Interpreter};

use super::TracerEip3155;
use crate::{
    blockchain::BlockHash,
    debug::ExtendedContext,
    instruction::InspectsInstructionWithJournal,
    state::{DatabaseComponents, State, WrapDatabaseRef},
};

/// Trait for retrieving a mutable reference to a [`TracerEip3155`] instance.
pub trait Eip3155TracerGetter {
    /// Retrieves a mutable reference to a [`TracerEip3155`] instance.
    fn eip3155_tracer(&mut self) -> &mut TracerEip3155;
}

impl<'tracer, BlockchainT, StateT> Eip3155TracerGetter
    for Eip3155TracerContext<'tracer, BlockchainT, StateT>
{
    fn eip3155_tracer(&mut self) -> &mut TracerEip3155 {
        self.tracer
    }
}

impl<'tracer, InnerContextT, OuterContextT> Eip3155TracerGetter
    for ExtendedContext<InnerContextT, OuterContextT>
where
    OuterContextT: Eip3155TracerGetter,
{
    fn eip3155_tracer(&mut self) -> &mut TracerEip3155 {
        self.extension.eip3155_tracer()
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
