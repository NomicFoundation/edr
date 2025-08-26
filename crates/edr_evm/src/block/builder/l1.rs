use core::{fmt::Debug, marker::PhantomData};
use std::time::{SystemTime, UNIX_EPOCH};

use derive_where::derive_where;
use edr_eth::{
    block::{BlobGas, HeaderOverrides, PartialHeader},
    eips::{eip4844, eip7691},
    l1,
    log::{ExecutionLog, FilterLog},
    receipt::{BlockReceipt, ExecutionReceipt, TransactionReceipt},
    result::{ExecutionResult, ExecutionResultAndState},
    transaction::ExecutableTransaction as _,
    trie::{ordered_trie_root, KECCAK_NULL_RLP},
    withdrawal::Withdrawal,
    Address, Bloom, HashMap, B256, U256,
};
use revm::{precompile::PrecompileFn, Inspector};

use super::{BlockBuilder, BlockTransactionError, BlockTransactionErrorForChainSpec};
use crate::{
    block::builder::BlockInputs,
    blockchain::SyncBlockchain,
    config::CfgEnv,
    receipt::{ExecutionReceiptBuilder as _, ReceiptFactory},
    runtime::{dry_run, dry_run_with_inspector},
    spec::{ContextForChainSpec, RuntimeSpec, SyncRuntimeSpec},
    state::{
        AccountModifierFn, DatabaseComponents, StateCommit as _, StateDebug as _, StateDiff,
        SyncState, WrapDatabaseRef,
    },
    transaction::TransactionError,
    Block as _, BlockBuilderCreationError, EthLocalBlockForChainSpec, MineBlockResultAndState,
};

/// A builder for constructing Ethereum L1 blocks.
pub struct EthBlockBuilder<'builder, BlockchainErrorT, ChainSpecT, StateErrorT>
where
    ChainSpecT: RuntimeSpec,
{
    blockchain: &'builder dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateErrorT>,
    cfg: CfgEnv<ChainSpecT::Hardfork>,
    header: PartialHeader,
    parent_gas_limit: Option<u64>,
    receipts: Vec<TransactionReceipt<ChainSpecT::ExecutionReceipt<ExecutionLog>>>,
    state: Box<dyn SyncState<StateErrorT>>,
    state_diff: StateDiff,
    transactions: Vec<ChainSpecT::SignedTransaction>,
    transaction_results: Vec<ExecutionResult<ChainSpecT::HaltReason>>,
    withdrawals: Option<Vec<Withdrawal>>,
    custom_precompiles: &'builder HashMap<Address, PrecompileFn>,
}

impl<BlockchainErrorT, ChainSpecT, StateErrorT>
    EthBlockBuilder<'_, BlockchainErrorT, ChainSpecT, StateErrorT>
where
    ChainSpecT: RuntimeSpec,
{
    /// Retrieves the blockchain of the block builder.
    pub fn blockchain(&self) -> &dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateErrorT> {
        self.blockchain
    }

    /// Retrieves the config of the block builder.
    pub fn config(&self) -> &CfgEnv<ChainSpecT::Hardfork> {
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
        transaction: &ChainSpecT::SignedTransaction,
    ) -> Result<(), BlockTransactionErrorForChainSpec<BlockchainErrorT, ChainSpecT, StateErrorT>>
    {
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
            let max_blob_gas_per_block = if self.config().spec.into() >= l1::SpecId::PRAGUE {
                eip7691::MAX_BLOBS_PER_BLOCK_ELECTRA * eip4844::GAS_PER_BLOB
            } else {
                eip4844::MAX_BLOB_GAS_PER_BLOCK_CANCUN
            };

            if block_blob_gas_used + blob_gas_used > max_blob_gas_per_block {
                return Err(BlockTransactionError::ExceedsBlockBlobGasLimit);
            }
        }

        Ok(())
    }
}

impl<'builder, BlockchainErrorT, ChainSpecT, StateErrorT>
    EthBlockBuilder<'builder, BlockchainErrorT, ChainSpecT, StateErrorT>
where
    BlockchainErrorT: Send + std::error::Error,
    ChainSpecT: RuntimeSpec,
    StateErrorT: Send + std::error::Error,
{
    /// Creates a new instance.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn new(
        blockchain: &'builder dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateErrorT>,
        state: Box<dyn SyncState<StateErrorT>>,
        cfg: CfgEnv<ChainSpecT::Hardfork>,
        inputs: BlockInputs,
        mut overrides: HeaderOverrides<ChainSpecT::Hardfork>,
        custom_precompiles: &'builder HashMap<Address, PrecompileFn>,
    ) -> Result<Self, BlockBuilderCreationError<BlockchainErrorT, ChainSpecT::Hardfork, StateErrorT>>
    {
        let parent_block = blockchain
            .last_block()
            .map_err(BlockBuilderCreationError::Blockchain)?;

        let eth_hardfork = cfg.spec.into();
        if eth_hardfork < l1::SpecId::BYZANTIUM {
            return Err(BlockBuilderCreationError::UnsupportedHardfork(cfg.spec));
        } else if eth_hardfork >= l1::SpecId::SHANGHAI && inputs.withdrawals.is_none() {
            return Err(BlockBuilderCreationError::MissingWithdrawals);
        }

        let parent_header = parent_block.header();
        let parent_gas_limit = if overrides.gas_limit.is_none() {
            Some(parent_header.gas_limit)
        } else {
            None
        };

        overrides.parent_hash = Some(*parent_block.block_hash());
        let header = PartialHeader::new::<ChainSpecT>(
            cfg.spec,
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
        transaction: ChainSpecT::SignedTransaction,
    ) -> Result<(), BlockTransactionErrorForChainSpec<BlockchainErrorT, ChainSpecT, StateErrorT>>
    {
        self.validate_transaction(&transaction)?;

        let block = ChainSpecT::new_block_env(&self.header, self.cfg.spec.into());

        let receipt_builder =
            ChainSpecT::ReceiptBuilder::new_receipt_builder(&self.state, &transaction)
                .map_err(TransactionError::State)?;

        let transaction_result = dry_run::<_, ChainSpecT, _>(
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
        transaction: ChainSpecT::SignedTransaction,
        extension: &mut InspectorT,
    ) -> Result<(), BlockTransactionErrorForChainSpec<BlockchainErrorT, ChainSpecT, StateErrorT>>
    where
        InspectorT: for<'inspector> Inspector<
            ContextForChainSpec<
                ChainSpecT,
                WrapDatabaseRef<
                    DatabaseComponents<
                        &'inspector dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateErrorT>,
                        &'inspector dyn SyncState<StateErrorT>,
                    >,
                >,
            >,
        >,
    {
        self.validate_transaction(&transaction)?;

        let block = ChainSpecT::new_block_env(&self.header, self.cfg.spec.into());

        let receipt_builder =
            ChainSpecT::ReceiptBuilder::new_receipt_builder(&self.state, &transaction)
                .map_err(TransactionError::State)?;

        let transaction_result = dry_run_with_inspector::<_, ChainSpecT, _, _>(
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
        transaction: ChainSpecT::SignedTransaction,
        transaction_result: ExecutionResultAndState<ChainSpecT::HaltReason>,
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

impl<BlockchainErrorT, ChainSpecT, StateErrorT>
    EthBlockBuilder<'_, BlockchainErrorT, ChainSpecT, StateErrorT>
where
    BlockchainErrorT: std::error::Error,
    ChainSpecT: SyncRuntimeSpec,
    StateErrorT: std::error::Error,
{
    /// Finalizes the block, applying rewards to the state.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn finalize(
        mut self,
        receipt_factory: impl ReceiptFactory<
            ChainSpecT::ExecutionReceipt<FilterLog>,
            ChainSpecT::Hardfork,
            ChainSpecT::SignedTransaction,
            Output = ChainSpecT::BlockReceipt,
        >,
        rewards: Vec<(Address, u128)>,
    ) -> Result<
        MineBlockResultAndState<
            ChainSpecT::HaltReason,
            EthLocalBlockForChainSpec<ChainSpecT>,
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
        let block = EthLocalBlockForChainSpec::<ChainSpecT>::new(
            receipt_factory,
            self.cfg.spec,
            self.header,
            self.transactions,
            self.receipts,
            Vec::new(),
            self.withdrawals,
        );

        Ok(MineBlockResultAndState {
            block,
            state: self.state,
            state_diff: self.state_diff,
            transaction_results: self.transaction_results,
        })
    }
}

impl<'builder, BlockchainErrorT, ChainSpecT, StateErrorT> BlockBuilder<'builder, ChainSpecT>
    for EthBlockBuilder<'builder, BlockchainErrorT, ChainSpecT, StateErrorT>
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
        blockchain: &'builder dyn SyncBlockchain<
            ChainSpecT,
            Self::BlockchainError,
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
        transaction: ChainSpecT::SignedTransaction,
    ) -> Result<
        (),
        BlockTransactionErrorForChainSpec<Self::BlockchainError, ChainSpecT, Self::StateError>,
    > {
        self.add_transaction(transaction)
    }

    fn add_transaction_with_inspector<InspectorT>(
        &mut self,
        transaction: ChainSpecT::SignedTransaction,
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
                        &'inspector dyn SyncBlockchain<
                            ChainSpecT,
                            Self::BlockchainError,
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

/// Factory for creating [`crate::block::EthLocalBlock`]s for chain specs with a
/// [`BlockReceipt`].
#[derive_where(Default)]
pub struct EthBlockReceiptFactory<ExecutionReceiptT: ExecutionReceipt<Log = FilterLog>> {
    phantom: PhantomData<ExecutionReceiptT>,
}

impl<
        ExecutionReceiptT: ExecutionReceipt<Log = FilterLog>,
        HardforkT: Into<l1::SpecId>,
        SignedTransactionT,
    > ReceiptFactory<ExecutionReceiptT, HardforkT, SignedTransactionT>
    for EthBlockReceiptFactory<ExecutionReceiptT>
{
    type Output = BlockReceipt<ExecutionReceiptT>;

    fn create_receipt(
        &self,
        hardfork: HardforkT,
        _transaction: &SignedTransactionT,
        mut transaction_receipt: TransactionReceipt<ExecutionReceiptT>,
        block_hash: &B256,
        block_number: u64,
    ) -> Self::Output {
        // The JSON-RPC layer should not return the gas price as effective gas price for
        // receipts in pre-London hardforks.
        if hardfork.into() < l1::SpecId::LONDON {
            transaction_receipt.effective_gas_price = None;
        }

        BlockReceipt {
            inner: transaction_receipt,
            block_hash: *block_hash,
            block_number,
        }
    }
}
