use edr_chain_spec::{ChainHardfork, ChainSpec};

pub trait EvmBuilderTrait: ChainHardfork + ChainSpec {
    /// Type of the EVM being built.
    type Evm<
        DatabaseT: Database,
        InspectorT: Inspector<
            EthInstructionsContext<BlockT, TransactionT, HardforkT, DatabaseT, ChainContextT>,
            EthInterpreter,
        >,
    >: InspectEvm<
        Block = BlockT,
        Inspector = InspectorT,
        Tx = TransactionT,
        ExecutionResult = ExecutionResult<HaltReasonT>,
        State = EvmState,
        Error = EVMError<DatabaseT::Error, TransactionErrorT>,
    > + IntoEvmContext<
        BlockT,
        ChainContextT,
        DatabaseT,
        HardforkT,
        TransactionT
    >;

    /// Type of the precompile provider used in the EVM.
    type PrecompileProvider<DatabaseT: Database>: Default
        + PrecompileProvider<
            EthInstructionsContext<BlockT, TransactionT, HardforkT, DatabaseT, ChainContextT>,
            Output = InterpreterResult,
        >;

    fn evm_with_inspector<
        DatabaseT: Database,
        InspectorT: Inspector<
            EthInstructionsContext<BlockT, TransactionT, HardforkT, DatabaseT, ChainContextT>,
            EthInterpreter,
        >,
    >(
        db: DatabaseT,
        env: EvmEnvWithChainContext<BlockT, TransactionT, HardforkT, ChainContextT>,
        inspector: InspectorT,
    ) -> Self::Evm<DatabaseT, InspectorT>;
}
