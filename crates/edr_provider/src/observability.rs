use edr_eth::spec::{ChainSpec, HaltReasonTrait};
use edr_evm::{
    blockchain::BlockHash,
    interpreter::{EthInterpreter, Interpreter},
    spec::RuntimeSpec,
    state::{DatabaseComponents, JournaledState, State, WrapDatabaseRef},
    trace::{RawTracerFrame, TraceCollector, TraceCollectorContext, TraceCollectorMutGetter},
};

use crate::{
    console_log::{
        ConsoleLogCollector, ConsoleLogCollectorFrame, ConsoleLogCollectorMutGetter,
        ConsoleLogContext,
    },
    debugger::Debugger,
    mock::{MockerMutGetter, MockingContext, MockingFrame},
};

pub struct RuntimeObservabilityContext<'context, BlockchainT, HaltReasonT: HaltReasonTrait, StateT>
{
    console_logger: ConsoleLogContext<'context>,
    mocker: MockingContext<'context>,
    trace_collector: TraceCollectorContext<'context, BlockchainT, HaltReasonT, StateT>,
}

impl<'context, BlockchainT, HaltReasonT: HaltReasonTrait, StateT>
    RuntimeObservabilityContext<'context, BlockchainT, HaltReasonT, StateT>
{
    pub fn new(runtime_observer: &'context mut Debugger<HaltReasonT>) -> Self {
        Self {
            console_logger: ConsoleLogContext::new(&mut runtime_observer.console_logger),
            mocker: MockingContext::new(&mut runtime_observer.mocker),
            trace_collector: TraceCollectorContext::new(&mut runtime_observer.trace_collector),
        }
    }
}

impl<'context, BlockchainT, HaltReasonT: HaltReasonTrait, StateT> ConsoleLogCollectorMutGetter
    for RuntimeObservabilityContext<'context, BlockchainT, HaltReasonT, StateT>
{
    fn console_log_collector_mut(&mut self) -> &mut ConsoleLogCollector {
        self.console_logger.console_log_collector_mut()
    }
}

impl<'context, BlockchainT, HaltReasonT: HaltReasonTrait, StateT> MockerMutGetter
    for RuntimeObservabilityContext<'context, BlockchainT, HaltReasonT, StateT>
{
    fn mocker_mut(&mut self) -> &mut crate::mock::Mocker {
        self.mocker.mocker_mut()
    }
}

impl<'context, BlockchainT, HaltReasonT: HaltReasonTrait, StateT>
    TraceCollectorMutGetter<HaltReasonT>
    for RuntimeObservabilityContext<'context, BlockchainT, HaltReasonT, StateT>
{
    fn trace_collector_mut(&mut self) -> &mut TraceCollector<HaltReasonT> {
        self.trace_collector.trace_collector_mut()
    }
}
impl<'context, BlockchainT, HaltReasonT, StateT> InspectsInstructionWithJournal
    for RuntimeObservabilityContext<'context, BlockchainT, HaltReasonT, StateT>
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
        self.trace_collector
            .before_instruction_with_journal(interpreter, journal);
    }

    fn after_instruction_with_journal(
        &mut self,
        interpreter: &Interpreter<Self::InterpreterTypes>,
        journal: &Self::Journal,
    ) {
        self.trace_collector
            .after_instruction_with_journal(interpreter, journal);
    }
}

/// Helper type for a frame used to achieve runtime observability in EDR.
pub type RuntimeObservabilityFrame<BlockchainErrorT, ChainSpecT, ContextT, StateErrorT> =
    MockingFrame<
        ConsoleLogCollectorFrame<
            RawTracerFrame<
                <<ChainSpecT as RuntimeSpec>::Evm<BlockchainErrorT, ContextT, StateErrorT> as EvmSpec<BlockchainErrorT, ChainSpecT, ContextT, StateErrorT>>::Frame<
                    InspectableInstructionProvider<
                        ContextT,
                        EthInterpreter,
                        <<ChainSpecT as RuntimeSpec>::Evm<BlockchainErrorT, ContextT, StateErrorT> as EvmSpec<BlockchainErrorT, ChainSpecT, ContextT, StateErrorT>>::InstructionProvider,
                    >,
                    <<ChainSpecT as RuntimeSpec>::Evm<BlockchainErrorT, ContextT, StateErrorT> as EvmSpec<BlockchainErrorT, ChainSpecT, ContextT, StateErrorT>>::PrecompileProvider,
                >,
                <ChainSpecT as ChainSpec>::HaltReason,
            >
        >
    >;
