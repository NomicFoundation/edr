use alloy_primitives::{B256, U256};
use alloy_provider::Provider;
use alloy_rpc_types::Filter;
use alloy_sol_types::SolValue;
use foundry_evm_core::{
    evm_context::{
        split_context, BlockEnvTr, ChainContextTr, EvmBuilderTrait, EvmContext, HardforkTr,
        TransactionEnvTr, TransactionErrorTrait,
    },
    fork::{provider::ProviderBuilder, CreateFork},
};
use revm::context::result::HaltReasonTr;

use crate::{
    impl_is_pure_false, impl_is_pure_true, Cheatcode, CheatcodeBackend, CheatsCtxt, Result,
    Vm::{
        activeForkCall, allowCheatcodesCall, createFork_0Call, createFork_1Call, createFork_2Call,
        createSelectFork_0Call, createSelectFork_1Call, createSelectFork_2Call, eth_getLogsCall,
        isPersistentCall, makePersistent_0Call, makePersistent_1Call, makePersistent_2Call,
        makePersistent_3Call, revokePersistent_0Call, revokePersistent_1Call, rollFork_0Call,
        rollFork_1Call, rollFork_2Call, rollFork_3Call, rpcCall, selectForkCall, transact_0Call,
        transact_1Call, EthGetLogs,
    },
};

impl_is_pure_true!(activeForkCall);
impl Cheatcode for activeForkCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self {} = self;
        ccx.ecx
            .journaled_state
            .database
            .active_fork_id()
            .map(|id| id.abi_encode())
            .ok_or_else(|| fmt_err!("no active fork"))
    }
}

impl_is_pure_false!(createFork_0Call);
impl Cheatcode for createFork_0Call {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { urlOrAlias } = self;
        create_fork(ccx, urlOrAlias, None)
    }
}

impl_is_pure_true!(createFork_1Call);
impl Cheatcode for createFork_1Call {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self {
            urlOrAlias,
            blockNumber,
        } = self;
        create_fork(ccx, urlOrAlias, Some(blockNumber.saturating_to()))
    }
}

impl_is_pure_true!(createFork_2Call);
impl Cheatcode for createFork_2Call {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { urlOrAlias, txHash } = self;
        create_fork_at_transaction(ccx, urlOrAlias, txHash)
    }
}

impl_is_pure_false!(createSelectFork_0Call);
impl Cheatcode for createSelectFork_0Call {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { urlOrAlias } = self;
        create_select_fork(ccx, urlOrAlias, None)
    }
}

impl_is_pure_true!(createSelectFork_1Call);
impl Cheatcode for createSelectFork_1Call {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self {
            urlOrAlias,
            blockNumber,
        } = self;
        create_select_fork(ccx, urlOrAlias, Some(blockNumber.saturating_to()))
    }
}

impl_is_pure_true!(createSelectFork_2Call);
impl Cheatcode for createSelectFork_2Call {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { urlOrAlias, txHash } = self;
        create_select_fork_at_transaction(ccx, urlOrAlias, txHash)
    }
}

impl_is_pure_true!(rollFork_0Call);
impl Cheatcode for rollFork_0Call {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { blockNumber } = self;
        let (db, mut context) = split_context(ccx.ecx);
        db.roll_fork(None, (*blockNumber).to(), &mut context)?;
        Ok(Vec::default())
    }
}

impl_is_pure_true!(rollFork_1Call);
impl Cheatcode for rollFork_1Call {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { txHash } = self;
        let (db, mut context) = split_context(ccx.ecx);
        db.roll_fork_to_transaction(None, *txHash, &mut context)?;
        Ok(Vec::default())
    }
}

impl_is_pure_true!(rollFork_2Call);
impl Cheatcode for rollFork_2Call {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self {
            forkId,
            blockNumber,
        } = self;
        let (db, mut context) = split_context(ccx.ecx);
        db.roll_fork(Some(*forkId), (*blockNumber).to(), &mut context)?;
        Ok(Vec::default())
    }
}

impl_is_pure_true!(rollFork_3Call);
impl Cheatcode for rollFork_3Call {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { forkId, txHash } = self;
        let (db, mut context) = split_context(ccx.ecx);
        db.roll_fork_to_transaction(Some(*forkId), *txHash, &mut context)?;
        Ok(Vec::default())
    }
}

impl_is_pure_true!(selectForkCall);
impl Cheatcode for selectForkCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { forkId } = self;
        let (db, mut context) = split_context(ccx.ecx);
        db.select_fork(*forkId, &mut context)?;
        Ok(Vec::default())
    }
}

impl_is_pure_true!(transact_0Call);
impl Cheatcode for transact_0Call {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { txHash } = *self;
        let (db, context) = split_context(ccx.ecx);
        db.transact(
            None,
            txHash,
            ccx.state,
            context.to_owned_env_with_chain_context(),
            context.journaled_state,
        )?;
        Ok(Vec::default())
    }
}

impl_is_pure_true!(transact_1Call);
impl Cheatcode for transact_1Call {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { forkId, txHash } = *self;
        let (db, context) = split_context(ccx.ecx);
        db.transact(
            Some(forkId),
            txHash,
            ccx.state,
            context.to_owned_env_with_chain_context(),
            context.journaled_state,
        )?;
        Ok(Vec::default())
    }
}

impl_is_pure_true!(allowCheatcodesCall);
impl Cheatcode for allowCheatcodesCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { account } = self;
        ccx.ecx
            .journaled_state
            .database
            .allow_cheatcode_access(*account);
        Ok(Vec::default())
    }
}

impl_is_pure_true!(makePersistent_0Call);
impl Cheatcode for makePersistent_0Call {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { account } = self;
        ccx.ecx
            .journaled_state
            .database
            .add_persistent_account(*account);
        Ok(Vec::default())
    }
}

impl_is_pure_true!(makePersistent_1Call);
impl Cheatcode for makePersistent_1Call {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { account0, account1 } = self;
        ccx.ecx
            .journaled_state
            .database
            .add_persistent_account(*account0);
        ccx.ecx
            .journaled_state
            .database
            .add_persistent_account(*account1);
        Ok(Vec::default())
    }
}

impl_is_pure_true!(makePersistent_2Call);
impl Cheatcode for makePersistent_2Call {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self {
            account0,
            account1,
            account2,
        } = self;
        ccx.ecx
            .journaled_state
            .database
            .add_persistent_account(*account0);
        ccx.ecx
            .journaled_state
            .database
            .add_persistent_account(*account1);
        ccx.ecx
            .journaled_state
            .database
            .add_persistent_account(*account2);
        Ok(Vec::default())
    }
}

impl_is_pure_true!(makePersistent_3Call);
impl Cheatcode for makePersistent_3Call {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { accounts } = self;
        ccx.ecx
            .journaled_state
            .database
            .extend_persistent_accounts(accounts.iter().copied());
        Ok(Vec::default())
    }
}

impl_is_pure_true!(revokePersistent_0Call);
impl Cheatcode for revokePersistent_0Call {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { account } = self;
        ccx.ecx
            .journaled_state
            .database
            .remove_persistent_account(account);
        Ok(Vec::default())
    }
}

impl_is_pure_true!(revokePersistent_1Call);
impl Cheatcode for revokePersistent_1Call {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { accounts } = self;
        ccx.ecx
            .journaled_state
            .database
            .remove_persistent_accounts(accounts.iter().copied());
        Ok(Vec::default())
    }
}

impl_is_pure_true!(isPersistentCall);
impl Cheatcode for isPersistentCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { account } = self;
        Ok(ccx
            .ecx
            .journaled_state
            .database
            .is_persistent(account)
            .abi_encode())
    }
}

// Calls like `eth_getBlockByNumber` are impure
impl_is_pure_false!(rpcCall);
impl Cheatcode for rpcCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { method, params } = self;
        let url = ccx
            .ecx
            .journaled_state
            .database
            .active_fork_url()
            .ok_or_else(|| fmt_err!("no active fork URL found"))?;
        let provider = ProviderBuilder::new(&url).build()?;
        let params_json: serde_json::Value = serde_json::from_str(params)?;
        let result = edr_common::block_on(provider.raw_request(method.clone().into(), params_json))
            .map_err(|err| fmt_err!("{method:?}: {err}"))?;

        let result_as_tokens = crate::json::json_value_to_token(&result)
            .map_err(|err| fmt_err!("failed to parse result: {err}"))?;

        Ok(result_as_tokens.abi_encode())
    }
}

impl_is_pure_true!(eth_getLogsCall);
impl Cheatcode for eth_getLogsCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self {
            fromBlock,
            toBlock,
            target,
            topics,
        } = self;
        let (Ok(from_block), Ok(to_block)) = (u64::try_from(fromBlock), u64::try_from(toBlock))
        else {
            bail!("blocks in block range must be less than 2^64 - 1")
        };

        if topics.len() > 4 {
            bail!("topics array must contain at most 4 elements")
        }

        let url = ccx
            .ecx
            .journaled_state
            .database
            .active_fork_url()
            .ok_or_else(|| fmt_err!("no active fork URL found"))?;
        let provider = ProviderBuilder::new(&url).build()?;
        let mut filter = Filter::new()
            .address(*target)
            .from_block(from_block)
            .to_block(to_block);
        for (i, &topic) in topics.iter().enumerate() {
            filter.topics[i] = topic.into();
        }

        let logs = edr_common::block_on(provider.get_logs(&filter))
            .map_err(|e| fmt_err!("failed to get logs: {e}"))?;

        let eth_logs = logs
            .into_iter()
            .map(|log| EthGetLogs {
                emitter: log.address(),
                topics: log.topics().to_vec(),
                data: log.inner.data.data,
                blockHash: log.block_hash.unwrap_or_default(),
                blockNumber: log.block_number.unwrap_or_default(),
                transactionHash: log.transaction_hash.unwrap_or_default(),
                transactionIndex: log.transaction_index.unwrap_or_default(),
                logIndex: U256::from(log.log_index.unwrap_or_default()),
                removed: log.removed,
            })
            .collect::<Vec<_>>();

        Ok(eth_logs.abi_encode())
    }
}

/// Creates and then also selects the new fork
fn create_select_fork<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
    DatabaseT: CheatcodeBackend<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >,
>(
    ccx: &mut CheatsCtxt<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
        DatabaseT,
    >,
    url_or_alias: &str,
    block: Option<u64>,
) -> Result {
    let fork = create_fork_request(ccx, url_or_alias, block)?;
    let (db, mut context) = split_context(ccx.ecx);
    let id = db.create_select_fork(fork, &mut context)?;
    Ok(id.abi_encode())
}

/// Creates a new fork
fn create_fork<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
    DatabaseT: CheatcodeBackend<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >,
>(
    ccx: &mut CheatsCtxt<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
        DatabaseT,
    >,
    url_or_alias: &str,
    block: Option<u64>,
) -> Result {
    let fork = create_fork_request(ccx, url_or_alias, block)?;
    let id = ccx.ecx.journaled_state.database.create_fork(fork)?;
    Ok(id.abi_encode())
}

/// Creates and then also selects the new fork at the given transaction
fn create_select_fork_at_transaction<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
    DatabaseT: CheatcodeBackend<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >,
>(
    ccx: &mut CheatsCtxt<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
        DatabaseT,
    >,
    url_or_alias: &str,
    transaction: &B256,
) -> Result {
    let fork = create_fork_request(ccx, url_or_alias, None)?;
    let (db, mut context) = split_context(ccx.ecx);
    let id = db.create_select_fork_at_transaction(fork, *transaction, &mut context)?;
    Ok(id.abi_encode())
}

/// Creates a new fork at the given transaction
fn create_fork_at_transaction<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
    DatabaseT: CheatcodeBackend<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >,
>(
    ccx: &mut CheatsCtxt<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
        DatabaseT,
    >,
    url_or_alias: &str,
    transaction: &B256,
) -> Result {
    let fork = create_fork_request(ccx, url_or_alias, None)?;
    let (db, context) = split_context(ccx.ecx);
    let id = db.create_fork_at_transaction(fork, *transaction, context.chain_context)?;
    Ok(id.abi_encode())
}

/// Creates the request object for a new fork request
fn create_fork_request<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
    DatabaseT: CheatcodeBackend<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >,
>(
    ccx: &mut CheatsCtxt<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
        DatabaseT,
    >,
    url_or_alias: &str,
    block: Option<u64>,
) -> Result<CreateFork<BlockT, TxT, HardforkT>> {
    let url = ccx.state.config.rpc_url(url_or_alias)?;

    let mut evm_opts = ccx.state.config.evm_opts.clone();
    evm_opts.fork_block_number = block;

    let context: EvmContext<'_, _, _, _, _> = ccx.ecx.into();

    let fork = CreateFork {
        rpc_cache_path: ccx.state.config.rpc_cache_path.clone(),
        url,
        env: context.to_owned_env(),
        evm_opts,
    };
    Ok(fork)
}
