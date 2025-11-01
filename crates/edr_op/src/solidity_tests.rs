use edr_evm::{
    evm::{self, Evm},
    inspector::Inspector,
    interpreter::{EthInstructions, EthInterpreter},
    journal::Journal,
    state::Database,
};
use edr_solidity_tests::{
    evm_context::{EthInstructionsContext, EvmBuilderTrait, EvmEnvWithChainContext},
    revm::context::{LocalContext, TxEnv},
};
use op_revm::{
    precompiles::OpPrecompiles, L1BlockInfo, OpEvm, OpHaltReason, OpSpecId, OpTransaction,
    OpTransactionError,
};

/// Type implementing the [`EvmBuilderTrait`] for the OP EVM.
pub struct OpEvmBuilder;

impl
    EvmBuilderTrait<
        edr_chain_l1::BlockEnv,
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
                edr_chain_l1::BlockEnv,
                OpTransaction<TxEnv>,
                OpSpecId,
                DatabaseT,
                L1BlockInfo,
            >,
            EthInterpreter,
        >,
    > = OpEvm<
        EthInstructionsContext<
            edr_chain_l1::BlockEnv,
            OpTransaction<TxEnv>,
            OpSpecId,
            DatabaseT,
            L1BlockInfo,
        >,
        InspectorT,
        EthInstructions<
            EthInterpreter,
            EthInstructionsContext<
                edr_chain_l1::BlockEnv,
                OpTransaction<TxEnv>,
                OpSpecId,
                DatabaseT,
                L1BlockInfo,
            >,
        >,
        Self::PrecompileProvider<DatabaseT>,
    >;

    type PrecompileProvider<DatabaseT: Database> = OpPrecompiles;

    fn evm_with_journal_and_inspector<
        DatabaseT: Database,
        InspectorT: Inspector<
            EthInstructionsContext<
                edr_chain_l1::BlockEnv,
                OpTransaction<TxEnv>,
                OpSpecId,
                DatabaseT,
                L1BlockInfo,
            >,
            EthInterpreter,
        >,
    >(
        journal: Journal<DatabaseT>,
        env: EvmEnvWithChainContext<
            edr_chain_l1::BlockEnv,
            OpTransaction<TxEnv>,
            OpSpecId,
            L1BlockInfo,
        >,
        inspector: InspectorT,
    ) -> Self::Evm<DatabaseT, InspectorT> {
        let context = evm::Context {
            tx: env.tx,
            block: env.block,
            cfg: env.cfg,
            journaled_state: journal,
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
