use core::{fmt::Debug, marker::PhantomData};
use std::time::{SystemTime, UNIX_EPOCH};

use alloy_eips::eip7840::BlobParams;
use edr_block_api::Block as _;
use edr_block_builder_api::{
    BlockBuilder, BlockBuilderCreationError, BlockInputs, BlockTransactionError,
    BuiltBlockAndState, CfgEnv, Context, DatabaseComponents, ExecutionResult, Journal,
    PrecompileFn, SyncBlockchain, WrapDatabaseRef,
};
use edr_block_header::{BlobGas, BlockConfig, HeaderOverrides, PartialHeader, Withdrawal};
use edr_block_local::EthLocalBlock;
use edr_chain_spec::{EvmSpecId, ExecutableTransaction as _, TransactionValidation};
use edr_evm_spec::{DatabaseComponentError, ExecutionResultAndState, Inspector, TransactionError};
use edr_primitives::{Address, Bloom, HashMap, B256, KECCAK_NULL_RLP, U256};
use edr_receipt::{
    log::{ExecutionLog, FilterLog},
    ExecutionReceipt, TransactionReceipt,
};
use edr_state_api::{AccountModifierFn, StateDiff, SyncState};
use edr_trie::ordered_trie_root;

/// A builder for constructing Ethereum L1 blocks.
pub struct EthBlockBuilder<
    'builder,
    BlockReceiptT: Send + Sync,
    BlockT: ?Sized,
    BlockchainErrorT: Debug + Send,
    ExecutionReceiptT: ExecutionReceipt<Log = ExecutionLog>,
    HaltReasonT,
    HardforkT: Send + Sync,
    LocalBlockT: Send + Sync,
    SignedTransactionT: Send + Sync,
    StateErrorT,
> {
    blockchain: &'builder dyn SyncBlockchain<
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
        StateErrorT,
    >,
    cfg: CfgEnv<HardforkT>,
    header: PartialHeader,
    parent_gas_limit: Option<u64>,
    receipts: Vec<TransactionReceipt<ExecutionReceiptT>>,
    state: Box<dyn SyncState<StateErrorT>>,
    state_diff: StateDiff,
    transactions: Vec<SignedTransactionT>,
    transaction_results: Vec<ExecutionResult<HaltReasonT>>,
    withdrawals: Option<Vec<Withdrawal>>,
    custom_precompiles: &'builder HashMap<Address, PrecompileFn>,
}

impl<
        BlockReceiptT: Send + Sync,
        BlockT: ?Sized,
        BlockchainErrorT: Debug + Send,
        ExecutionReceiptT: ExecutionReceipt<Log = ExecutionLog>,
        HaltReasonT,
        HardforkT: Send + Sync,
        LocalBlockT: Send + Sync,
        SignedTransactionT: Send + Sync,
        StateErrorT,
    >
    EthBlockBuilder<
        '_,
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        ExecutionReceiptT,
        HaltReasonT,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
        StateErrorT,
    >
{
    /// Retrieves the blockchain of the block builder.
    pub fn blockchain(
        &self,
    ) -> &dyn SyncBlockchain<
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
        StateErrorT,
    > {
        self.blockchain
    }

    /// Retrieves the config of the block builder.
    pub fn config(&self) -> &CfgEnv<HardforkT> {
        &self.cfg
    }

    /// Retrieves the header of the block builder.
    pub fn header(&self) -> &PartialHeader {
        &self.header
    }

    /// Retrieves the amount of gas used in the block, so far.
    pub fn gas_used(&self) -> u64 {
        self.header.gas_used
    }

    /// Retrieves the amount of gas left in the block.
    pub fn gas_remaining(&self) -> u64 {
        self.header.gas_limit - self.gas_used()
    }

    /// Retrieves the state of the block builder.
    pub fn state(&self) -> &dyn SyncState<StateErrorT> {
        self.state.as_ref()
    }

    fn validate_transaction(
        &self,
        transaction: &SignedTransactionT,
    ) -> Result<
        (),
        BlockTransactionError<
            DatabaseComponentError<BlockchainErrorT, StateErrorT>,
            <SignedTransactionT as TransactionValidation>::ValidationError,
        >,
    > {
        // The transaction's gas limit cannot be greater than the remaining gas in the
        // block
        if transaction.gas_limit() > self.gas_remaining() {
            return Err(BlockTransactionError::ExceedsBlockGasLimit);
        }

        let blob_gas_used = transaction.total_blob_gas().unwrap_or_default();
        if let Some(BlobGas {
            gas_used: block_blob_gas_used,
            ..
        }) = self.header.blob_gas.as_ref()
        {
            let blob_params = if self.config().spec.into() >= EvmSpecId::PRAGUE {
                BlobParams::prague()
            } else {
                BlobParams::cancun()
            };

            if block_blob_gas_used + blob_gas_used > blob_params.max_blob_gas_per_block() {
                return Err(BlockTransactionError::ExceedsBlockBlobGasLimit);
            }
        }

        Ok(())
    }
}

impl<
        'builder,
        BlockReceiptT: Send + Sync,
        BlockT: ?Sized,
        BlockchainErrorT: Debug + Send,
        ExecutionReceiptT: ExecutionReceipt<Log = ExecutionLog>,
        HaltReasonT,
        HardforkT: Send + Sync,
        LocalBlockT: Send + Sync,
        SignedTransactionT: Send + Sync,
        StateErrorT,
    >
    EthBlockBuilder<
        'builder,
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        ExecutionReceiptT,
        HaltReasonT,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
        StateErrorT,
    >
// where
//     BlockchainErrorT: Send + std::error::Error,
//     StateErrorT: Send + std::error::Error,
{
    /// Creates a new instance.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn new(
        blockchain: &'builder dyn SyncBlockchain<
            BlockReceiptT,
            BlockT,
            BlockchainErrorT,
            HardforkT,
            LocalBlockT,
            SignedTransactionT,
            StateErrorT,
        >,
        state: Box<dyn SyncState<StateErrorT>>,
        cfg: CfgEnv<HardforkT>,
        inputs: BlockInputs,
        mut overrides: HeaderOverrides<HardforkT>,
        custom_precompiles: &'builder HashMap<Address, PrecompileFn>,
    ) -> Result<
        Self,
        BlockBuilderCreationError<DatabaseComponentError<BlockchainErrorT, StateErrorT>, HardforkT>,
    > {
        let parent_block = blockchain
            .last_block()
            .map_err(BlockBuilderCreationError::Blockchain)?;

        let eth_hardfork = cfg.spec.into();
        if eth_hardfork < EvmSpecId::BYZANTIUM {
            return Err(BlockBuilderCreationError::UnsupportedHardfork(cfg.spec));
        } else if eth_hardfork >= EvmSpecId::SHANGHAI && inputs.withdrawals.is_none() {
            return Err(BlockBuilderCreationError::MissingWithdrawals);
        }

        let parent_header = parent_block.block_header();
        let parent_gas_limit = if overrides.gas_limit.is_none() {
            Some(parent_header.gas_limit)
        } else {
            None
        };

        overrides.parent_hash = Some(*parent_block.block_hash());
        let header = PartialHeader::new(
            BlockConfig {
                base_fee_params: base_fee_params_for::<ChainSpecT>(cfg.chain_id),
                hardfork: cfg.spec,
                min_ethash_difficulty: inputs.min_ethash_difficulty,
            },
            overrides,
            Some(parent_header),
            &inputs.ommers,
            inputs.withdrawals.as_ref(),
        );

        Ok(Self {
            blockchain,
            cfg,
            header,
            parent_gas_limit,
            receipts: Vec::new(),
            state,
            state_diff: StateDiff::default(),
            transactions: Vec::new(),
            transaction_results: Vec::new(),
            withdrawals: inputs.withdrawals,
            custom_precompiles,
        })
    }

    /// Tries to add a transaction to the block.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn add_transaction(
        &mut self,
        transaction: SignedTransactionT,
    ) -> Result<
        (),
        BlockTransactionError<
            DatabaseComponentError<BlockchainErrorT, StateErrorT>,
            <SignedTransactionT as TransactionValidation>::ValidationError,
        >,
    > {
        self.validate_transaction(&transaction)?;

        let block = ChainSpecT::new_block_env(&self.header, self.cfg.spec.into());

        let receipt_builder =
            ChainSpecT::ReceiptBuilder::new_receipt_builder(&self.state, &transaction)
                .map_err(TransactionError::State)?;

        let transaction_result = dry_run(
            self.blockchain,
            &self.state,
            self.cfg.clone(),
            transaction.clone(),
            block,
            self.custom_precompiles,
        )?;

        self.add_transaction_result(receipt_builder, transaction, transaction_result);

        Ok(())
    }
    /// Tries to add a transaction to the block.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn add_transaction_with_inspector<InspectorT>(
        &mut self,
        transaction: SignedTransactionT,
        extension: &mut InspectorT,
    ) -> Result<
        (),
        BlockTransactionError<
            DatabaseComponentError<BlockchainErrorT, StateErrorT>,
            <SignedTransactionT as TransactionValidation>::ValidationError,
        >,
    >
    where
        InspectorT: for<'inspector> Inspector<
            Context<
                BlockEnvT,
                SignedTransactionT,
                CfgEnv<HardforkT>,
                WrapDatabaseRef<
                    DatabaseComponents<
                        &'inspector dyn SyncBlockchain<
                            BlockReceiptT,
                            BlockT,
                            Self::BlockchainError,
                            HardforkT,
                            LocalBlockT,
                            SignedTransactionT,
                            Self::StateError,
                        >,
                        &'inspector dyn SyncState<Self::StateError>,
                    >,
                >,
                Journal<
                    WrapDatabaseRef<
                        DatabaseComponents<
                            &'inspector dyn SyncBlockchain<
                                BlockReceiptT,
                                BlockT,
                                Self::BlockchainError,
                                HardforkT,
                                LocalBlockT,
                                SignedTransactionT,
                                Self::StateError,
                            >,
                            &'inspector dyn SyncState<Self::StateError>,
                        >,
                    >,
                >,
                (),
            >,
        >,
    {
        self.validate_transaction(&transaction)?;

        let block = ChainSpecT::new_block_env(&self.header, self.cfg.spec.into());

        let receipt_builder =
            ChainSpecT::ReceiptBuilder::new_receipt_builder(&self.state, &transaction)
                .map_err(TransactionError::State)?;

        let transaction_result = dry_run_with_inspector(
            self.blockchain,
            self.state.as_ref(),
            self.cfg.clone(),
            transaction.clone(),
            block,
            self.custom_precompiles,
            extension,
        )
        .map_err(BlockTransactionError::from)?;

        self.add_transaction_result(receipt_builder, transaction, transaction_result);

        Ok(())
    }

    fn add_transaction_result(
        &mut self,
        receipt_builder: ChainSpecT::ReceiptBuilder,
        transaction: SignedTransactionT,
        transaction_result: ExecutionResultAndState<HaltReasonT>,
    ) {
        let ExecutionResultAndState {
            result: transaction_result,
            state: state_diff,
        } = transaction_result;

        self.state_diff.apply_diff(state_diff.clone());

        self.state.commit(state_diff);

        self.header.gas_used += transaction_result.gas_used();

        if let Some(BlobGas { gas_used, .. }) = self.header.blob_gas.as_mut() {
            let blob_gas_used = transaction.total_blob_gas().unwrap_or_default();
            *gas_used += blob_gas_used;
        }

        let receipt = receipt_builder.build_receipt(
            &self.header,
            &transaction,
            &transaction_result,
            self.cfg.spec,
        );
        let receipt = TransactionReceipt::new(
            receipt,
            &transaction,
            &transaction_result,
            self.transactions.len() as u64,
            self.header.base_fee.unwrap_or(0),
            self.cfg.spec,
        );
        self.receipts.push(receipt);

        self.transactions.push(transaction);
        self.transaction_results.push(transaction_result);
    }
}

impl<
        BlockReceiptT: Send + Sync,
        BlockT: ?Sized,
        BlockchainErrorT: Debug + Send,
        ExecutionReceiptT: ExecutionReceipt<Log = ExecutionLog>,
        HaltReasonT,
        HardforkT: Send + Sync,
        LocalBlockT: Send + Sync,
        SignedTransactionT: Send + Sync,
        StateErrorT,
    >
    EthBlockBuilder<
        '_,
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        ExecutionReceiptT,
        HaltReasonT,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
        StateErrorT,
    >
{
    /// Finalizes the block, applying rewards to the state.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn finalize(
        mut self,
        rewards: Vec<(Address, u128)>,
    ) -> Result<
        BuiltBlockAndState<
            HaltReasonT,
            EthLocalBlock<BlockReceiptT, ExecutionReceiptT, HardforkT, SignedTransactionT>,
            StateErrorT,
        >,
        StateErrorT,
    > {
        for (address, reward) in rewards {
            if reward > 0 {
                let account_info = self.state.modify_account(
                    address,
                    AccountModifierFn::new(Box::new(move |balance, _nonce, _code| {
                        *balance += U256::from(reward);
                    })),
                )?;

                self.state_diff.apply_account_change(address, account_info);
            }
        }

        if let Some(gas_limit) = self.parent_gas_limit {
            self.header.gas_limit = gas_limit;
        }

        self.header.logs_bloom = {
            let mut logs_bloom = Bloom::ZERO;
            self.receipts.iter().for_each(|receipt| {
                logs_bloom.accrue_bloom(receipt.logs_bloom());
            });
            logs_bloom
        };

        self.header.receipts_root = ordered_trie_root(self.receipts.iter().map(alloy_rlp::encode));

        // Only set the state root if it wasn't specified during construction
        if self.header.state_root == KECCAK_NULL_RLP {
            self.header.state_root = self
                .state
                .state_root()
                .expect("Must be able to calculate state root");
        }

        // Only set the timestamp if it wasn't specified during construction
        if self.header.timestamp == 0 {
            self.header.timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Current time must be after unix epoch")
                .as_secs();
        }

        // TODO: handle ommers
        let block = EthLocalBlock::new(
            (),
            self.cfg.spec,
            self.header,
            self.transactions,
            self.receipts,
            Vec::new(),
            self.withdrawals,
        );

        Ok(BuiltBlockAndState {
            block,
            state: self.state,
            state_diff: self.state_diff,
            transaction_results: self.transaction_results,
        })
    }
}

impl<'builder, BlockchainErrorT, ChainSpecT, StateErrorT>
    BlockBuilder<
        'builder,
        BlockEnvT,
        BlockReceiptT: Send + Sync,
        BlockT: ?Sized,
        ContextT,
        HaltReasonT: HaltReasonTrait,
        HardforkT: Send + Sync,
        LocalBlockT: Send + Sync,
        SignedTransactionT: TransactionValidation + Send + Sync,
    >
    for EthBlockBuilder<
        'builder,
        BlockReceiptT: Send + Sync,
        BlockT: ?Sized,
        BlockchainErrorT: Debug + Send,
        ExecutionReceiptT: ExecutionReceipt<Log = ExecutionLog>,
        HaltReasonT,
        HardforkT: Send + Sync,
        LocalBlockT: Send + Sync,
        SignedTransactionT: Send + Sync,
        StateErrorT,
    >
where
    BlockchainErrorT: Send + std::error::Error,
    ChainSpecT: SyncRuntimeSpec<
        BlockReceiptFactory: Default,
        Hardfork: Debug,
        LocalBlock: From<EthLocalBlockForChainSpec<ChainSpecT>>,
    >,
    StateErrorT: Send + std::error::Error,
{
    type BlockchainError = BlockchainErrorT;

    type StateError = StateErrorT;

    fn new_block_builder(
        blockchain: &'builder dyn SyncBlockchainForChainSpec<
            Self::BlockchainError,
            ChainSpecT,
            Self::StateError,
        >,
        state: Box<dyn SyncState<Self::StateError>>,
        cfg: CfgEnv<ChainSpecT::Hardfork>,
        inputs: BlockInputs,
        overrides: HeaderOverrides<ChainSpecT::Hardfork>,
        custom_precompiles: &'builder HashMap<Address, PrecompileFn>,
    ) -> Result<
        Self,
        BlockBuilderCreationError<Self::BlockchainError, ChainSpecT::Hardfork, Self::StateError>,
    > {
        Self::new(
            blockchain,
            state,
            cfg,
            inputs,
            overrides,
            custom_precompiles,
        )
    }

    fn block_receipt_factory(&self) -> ChainSpecT::BlockReceiptFactory {
        ChainSpecT::BlockReceiptFactory::default()
    }

    fn header(&self) -> &PartialHeader {
        self.header()
    }

    fn add_transaction(
        &mut self,
        transaction: SignedTransactionT,
    ) -> Result<
        (),
        BlockTransactionErrorForChainSpec<Self::BlockchainError, ChainSpecT, Self::StateError>,
    > {
        self.add_transaction(transaction)
    }

    fn add_transaction_with_inspector<InspectorT>(
        &mut self,
        transaction: SignedTransactionT,
        inspector: &mut InspectorT,
    ) -> Result<
        (),
        BlockTransactionErrorForChainSpec<Self::BlockchainError, ChainSpecT, Self::StateError>,
    >
    where
        InspectorT: for<'inspector> Inspector<
            ContextForChainSpec<
                ChainSpecT,
                WrapDatabaseRef<
                    DatabaseComponents<
                        &'inspector dyn SyncBlockchainForChainSpec<
                            Self::BlockchainError,
                            ChainSpecT,
                            Self::StateError,
                        >,
                        &'inspector dyn SyncState<Self::StateError>,
                    >,
                >,
            >,
        >,
    {
        self.add_transaction_with_inspector(transaction, inspector)
    }

    fn finalize(
        self,
        rewards: Vec<(Address, u128)>,
    ) -> Result<
        MineBlockResultAndState<ChainSpecT::HaltReason, ChainSpecT::LocalBlock, Self::StateError>,
        Self::StateError,
    > {
        let receipt_factory = self.block_receipt_factory();

        let MineBlockResultAndState {
            block,
            state,
            state_diff,
            transaction_results,
        } = self.finalize(receipt_factory, rewards)?;

        Ok(MineBlockResultAndState {
            block: block.into(),
            state,
            state_diff,
            transaction_results,
        })
    }
}
