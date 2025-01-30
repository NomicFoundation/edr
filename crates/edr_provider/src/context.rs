use edr_eth::{spec::HaltReasonTrait, Address, HashMap};
use edr_evm::{
    blockchain::BlockHash,
    debug_trace::{Eip3155TracerContext, Eip3155TracerMutGetter, TracerEip3155},
    instruction::InspectsInstructionWithJournal,
    interpreter::{EthInterpreter, Interpreter},
    precompile::CustomPrecompilesGetter,
    state::{DatabaseComponents, JournaledState, State, WrapDatabaseRef},
    trace::{TraceCollector, TraceCollectorContext, TraceCollectorMutGetter},
};
use revm_precompile::PrecompileFn;

/// EIP-3155 and raw tracers, alongside custom precompiles.
pub struct Eip3155AndRawTracersContextWithPrecompiles<
    'tracer,
    BlockchainT,
    HaltReasonT: HaltReasonTrait,
    StateT,
> {
    eip3155: Eip3155TracerContext<'tracer, BlockchainT, StateT>,
    raw: TraceCollectorContext<'tracer, BlockchainT, HaltReasonT, StateT>,
    custom_precompiles: &'tracer HashMap<Address, PrecompileFn>,
}

impl<'tracer, BlockchainT, HaltReasonT: HaltReasonTrait, StateT>
    Eip3155AndRawTracersContextWithPrecompiles<'tracer, BlockchainT, HaltReasonT, StateT>
{
    /// Creates a new instance.
    pub fn new(
        eip3155: &'tracer mut TracerEip3155,
        raw: &'tracer mut TraceCollector<HaltReasonT>,
        custom_precompiles: &'tracer HashMap<Address, PrecompileFn>,
    ) -> Self {
        Self {
            eip3155: Eip3155TracerContext::new(eip3155),
            raw: TraceCollectorContext::new(raw),
            custom_precompiles,
        }
    }
}

impl<BlockchainT, HaltReasonT, StateT> CustomPrecompilesGetter
    for Eip3155AndRawTracersContextWithPrecompiles<'_, BlockchainT, HaltReasonT, StateT>
where
    HaltReasonT: HaltReasonTrait,
{
    fn custom_precompiles(&self) -> HashMap<Address, PrecompileFn> {
        self.custom_precompiles.clone()
    }
}

impl<BlockchainT, HaltReasonT, StateT> Eip3155TracerMutGetter
    for Eip3155AndRawTracersContextWithPrecompiles<'_, BlockchainT, HaltReasonT, StateT>
where
    HaltReasonT: HaltReasonTrait,
{
    fn eip3155_tracer_mut(&mut self) -> &mut TracerEip3155 {
        self.eip3155.eip3155_tracer_mut()
    }
}

impl<BlockchainT, HaltReasonT, StateT> InspectsInstructionWithJournal
    for Eip3155AndRawTracersContextWithPrecompiles<'_, BlockchainT, HaltReasonT, StateT>
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

impl<BlockchainT, HaltReasonT, StateT> TraceCollectorMutGetter<HaltReasonT>
    for Eip3155AndRawTracersContextWithPrecompiles<'_, BlockchainT, HaltReasonT, StateT>
where
    HaltReasonT: HaltReasonTrait,
{
    fn trace_collector_mut(&mut self) -> &mut TraceCollector<HaltReasonT> {
        self.raw.trace_collector_mut()
    }
}
