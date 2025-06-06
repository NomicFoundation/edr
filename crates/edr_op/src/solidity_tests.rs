use edr_eth::l1;
use edr_evm::{
    evm::{self, Evm},
    inspector::Inspector,
    interpreter::{EthInstructions, EthInterpreter},
    journal::{Journal, JournalEntry},
    state::Database,
};
use edr_solidity_tests::{
    evm_context::{EthInstructionsContext, EvmBuilderTrait, EvmEnv},
    revm::context::TxEnv,
};
use op_revm::{
    precompiles::OpPrecompiles, L1BlockInfo, OpEvm, OpHaltReason, OpSpecId, OpTransaction,
};

pub struct OpEvmBuilder;

impl EvmBuilderTrait<l1::BlockEnv, L1BlockInfo, OpHaltReason, OpSpecId, OpTransaction<TxEnv>>
    for OpEvmBuilder
{
    type Evm<
        DatabaseT: Database,
        InspectorT: Inspector<
            EthInstructionsContext<
                l1::BlockEnv,
                OpTransaction<TxEnv>,
                OpSpecId,
                DatabaseT,
                L1BlockInfo,
            >,
            EthInterpreter,
        >,
    > = OpEvm<
        EthInstructionsContext<
            l1::BlockEnv,
            OpTransaction<TxEnv>,
            OpSpecId,
            DatabaseT,
            L1BlockInfo,
        >,
        InspectorT,
        EthInstructions<
            EthInterpreter,
            EthInstructionsContext<
                l1::BlockEnv,
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
                l1::BlockEnv,
                OpTransaction<TxEnv>,
                OpSpecId,
                DatabaseT,
                L1BlockInfo,
            >,
            EthInterpreter,
        >,
    >(
        db: DatabaseT,
        env: EvmEnv<l1::BlockEnv, OpTransaction<TxEnv>, OpSpecId>,
        inspector: InspectorT,
        chain: L1BlockInfo,
    ) -> Self::Evm<DatabaseT, InspectorT> {
        let mut journaled_state = Journal::<_, JournalEntry>::new(db);
        journaled_state.set_spec_id(env.cfg.spec);

        let context = evm::Context {
            tx: env.tx,
            block: env.block,
            cfg: env.cfg,
            journaled_state,
            chain,
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
