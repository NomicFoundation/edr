use edr_evm_spec::{
    handler::EthInstructions, interpreter::EthInterpreter, Context, Database, Evm, Inspector,
    Journal, JournalEntry,
};
use edr_solidity_tests::{
    evm_context::{EthInstructionsContext, EvmBuilderTrait, EvmEnvWithChainContext},
    revm::context::{LocalContext, TxEnv},
};
use op_revm::{
    precompiles::OpPrecompiles, L1BlockInfo, OpEvm, OpHaltReason, OpSpecId, OpTransaction,
    OpTransactionError,
};
use revm_context::{BlockEnv, JournalTr as _};

/// Type implementing the [`EvmBuilderTrait`] for the OP EVM.
pub struct OpEvmBuilder;

impl
    EvmBuilderTrait<
        BlockEnv,
        L1BlockInfo,
        OpHaltReason,
        OpSpecId,
        OpTransactionError,
        OpTransaction<TxEnv>,
    > for OpEvmBuilder
{
    type Evm<
        DatabaseT: Database,
        InspectorT: Inspector<
            EthInstructionsContext<
                BlockEnv,
                OpTransaction<TxEnv>,
                OpSpecId,
                DatabaseT,
                L1BlockInfo,
            >,
            EthInterpreter,
        >,
    > = OpEvm<
        EthInstructionsContext<BlockEnv, OpTransaction<TxEnv>, OpSpecId, DatabaseT, L1BlockInfo>,
        InspectorT,
        EthInstructions<
            EthInterpreter,
            EthInstructionsContext<
                BlockEnv,
                OpTransaction<TxEnv>,
                OpSpecId,
                DatabaseT,
                L1BlockInfo,
            >,
        >,
        Self::PrecompileProvider<DatabaseT>,
    >;

    type PrecompileProvider<DatabaseT: Database> = OpPrecompiles;

    fn evm_with_inspector<
        DatabaseT: Database,
        InspectorT: Inspector<
            EthInstructionsContext<
                BlockEnv,
                OpTransaction<TxEnv>,
                OpSpecId,
                DatabaseT,
                L1BlockInfo,
            >,
            EthInterpreter,
        >,
    >(
        db: DatabaseT,
        env: EvmEnvWithChainContext<BlockEnv, OpTransaction<TxEnv>, OpSpecId, L1BlockInfo>,
        inspector: InspectorT,
    ) -> Self::Evm<DatabaseT, InspectorT> {
        let mut journaled_state = Journal::<DatabaseT, JournalEntry>::new(db);
        journaled_state.set_spec_id(env.cfg.spec.into());

        let context = Context {
            tx: env.tx,
            block: env.block,
            cfg: env.cfg,
            journaled_state,
            chain: env.chain_context,
            local: LocalContext::default(),
            error: Ok(()),
        };

        OpEvm(Evm::new_with_inspector(
            context,
            inspector,
            EthInstructions::default(),
            OpPrecompiles::default(),
        ))
    }
}
