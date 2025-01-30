use edr_eth::{spec::HaltReasonTrait, Address, HashMap};
use edr_evm::{
    blockchain::BlockHash,
    instruction::InspectsInstructionWithJournal,
    interpreter::{EthInterpreter, Interpreter},
    precompile::CustomPrecompilesGetter,
    state::{DatabaseComponents, JournaledState, State, WrapDatabaseRef},
    trace::{TraceCollector, TraceCollectorContext, TraceCollectorMutGetter},
};
use revm_precompile::PrecompileFn;

use super::Debugger;
use crate::{
    console_log::{ConsoleLogCollector, ConsoleLogCollectorMutGetter, ConsoleLogContext},
    mock::{Mocker, MockerMutGetter, MockingContext},
};

/// Context for [`Debugger`].
pub struct DebuggerContext<'context, BlockchainT, HaltReasonT: HaltReasonTrait, StateT> {
    console: ConsoleLogContext<'context>,
    mocker: MockingContext<'context>,
    raw: TraceCollectorContext<'context, BlockchainT, HaltReasonT, StateT>,
}

impl<'tracer, BlockchainT, HaltReasonT: HaltReasonTrait, StateT>
    DebuggerContext<'tracer, BlockchainT, HaltReasonT, StateT>
{
    /// Creates a new instance.
    pub fn new(debugger: &'tracer mut Debugger<HaltReasonT>) -> Self {
        Self {
            console: ConsoleLogContext::new(&mut debugger.console_logger),
            mocker: MockingContext::new(&mut debugger.mocker),
            raw: TraceCollectorContext::new(&mut debugger.trace_collector),
        }
    }
}

impl<BlockchainT, HaltReasonT, StateT> ConsoleLogCollectorMutGetter
    for DebuggerContext<'_, BlockchainT, HaltReasonT, StateT>
where
    HaltReasonT: HaltReasonTrait,
{
    fn console_log_collector_mut(&mut self) -> &mut ConsoleLogCollector {
        self.console.console_log_collector_mut()
    }
}

impl<BlockchainT, HaltReasonT, StateT> InspectsInstructionWithJournal
    for DebuggerContext<'_, BlockchainT, HaltReasonT, StateT>
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
        self.raw
            .before_instruction_with_journal(interpreter, journal);
    }

    fn after_instruction_with_journal(
        &mut self,
        interpreter: &Interpreter<Self::InterpreterTypes>,
        journal: &Self::Journal,
    ) {
        self.raw
            .after_instruction_with_journal(interpreter, journal);
    }
}

impl<BlockchainT, HaltReasonT, StateT> MockerMutGetter
    for DebuggerContext<'_, BlockchainT, HaltReasonT, StateT>
where
    HaltReasonT: HaltReasonTrait,
{
    fn mocker_mut(&mut self) -> &mut Mocker {
        self.mocker.mocker_mut()
    }
}

impl<BlockchainT, HaltReasonT, StateT> TraceCollectorMutGetter<HaltReasonT>
    for DebuggerContext<'_, BlockchainT, HaltReasonT, StateT>
where
    HaltReasonT: HaltReasonTrait,
{
    fn trace_collector_mut(&mut self) -> &mut TraceCollector<HaltReasonT> {
        self.raw.trace_collector_mut()
    }
}

/// Context for [`Debugger`], alongside custom precompiles.
pub struct DebuggerContextWithPrecompiles<
    'context,
    BlockchainT,
    HaltReasonT: HaltReasonTrait,
    StateT,
> {
    console: ConsoleLogContext<'context>,
    mocker: MockingContext<'context>,
    raw: TraceCollectorContext<'context, BlockchainT, HaltReasonT, StateT>,
    custom_precompiles: &'context HashMap<Address, PrecompileFn>,
}

impl<'tracer, BlockchainT, HaltReasonT: HaltReasonTrait, StateT>
    DebuggerContextWithPrecompiles<'tracer, BlockchainT, HaltReasonT, StateT>
{
    /// Creates a new instance.
    pub fn new(
        debugger: &'tracer mut Debugger<HaltReasonT>,
        custom_precompiles: &'tracer HashMap<Address, PrecompileFn>,
    ) -> Self {
        Self {
            console: ConsoleLogContext::new(&mut debugger.console_logger),
            mocker: MockingContext::new(&mut debugger.mocker),
            raw: TraceCollectorContext::new(&mut debugger.trace_collector),
            custom_precompiles,
        }
    }
}

impl<BlockchainT, HaltReasonT, StateT> ConsoleLogCollectorMutGetter
    for DebuggerContextWithPrecompiles<'_, BlockchainT, HaltReasonT, StateT>
where
    HaltReasonT: HaltReasonTrait,
{
    fn console_log_collector_mut(&mut self) -> &mut ConsoleLogCollector {
        self.console.console_log_collector_mut()
    }
}

impl<BlockchainT, HaltReasonT, StateT> CustomPrecompilesGetter
    for DebuggerContextWithPrecompiles<'_, BlockchainT, HaltReasonT, StateT>
where
    HaltReasonT: HaltReasonTrait,
{
    fn custom_precompiles(&self) -> HashMap<Address, PrecompileFn> {
        self.custom_precompiles.clone()
    }
}

impl<BlockchainT, HaltReasonT, StateT> InspectsInstructionWithJournal
    for DebuggerContextWithPrecompiles<'_, BlockchainT, HaltReasonT, StateT>
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
        self.raw
            .before_instruction_with_journal(interpreter, journal);
    }

    fn after_instruction_with_journal(
        &mut self,
        interpreter: &Interpreter<Self::InterpreterTypes>,
        journal: &Self::Journal,
    ) {
        self.raw
            .after_instruction_with_journal(interpreter, journal);
    }
}

impl<BlockchainT, HaltReasonT, StateT> MockerMutGetter
    for DebuggerContextWithPrecompiles<'_, BlockchainT, HaltReasonT, StateT>
where
    HaltReasonT: HaltReasonTrait,
{
    fn mocker_mut(&mut self) -> &mut Mocker {
        self.mocker.mocker_mut()
    }
}

impl<BlockchainT, HaltReasonT, StateT> TraceCollectorMutGetter<HaltReasonT>
    for DebuggerContextWithPrecompiles<'_, BlockchainT, HaltReasonT, StateT>
where
    HaltReasonT: HaltReasonTrait,
{
    fn trace_collector_mut(&mut self) -> &mut TraceCollector<HaltReasonT> {
        self.raw.trace_collector_mut()
    }
}
