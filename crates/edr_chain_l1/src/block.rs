use core::{fmt::Debug, marker::PhantomData};
use std::time::{SystemTime, UNIX_EPOCH};

use edr_block_api::Block;
use edr_block_builder_api::{
    BlockBuilder, BlockBuilderCreationError, BlockFinalizeError, BlockInputs,
    BlockTransactionError, BlockTransactionErrorForChainSpec, Blockchain, BuiltBlockAndState,
    CfgEnv, DatabaseComponents, ExecutionResult, PrecompileFn, WrapDatabaseRef,
};
use edr_block_header::{
    blob_params_for_hardfork, BlobGas, BlockConfig, HeaderAndEvmSpec, HeaderOverrides,
    PartialHeader, Withdrawal,
};
use edr_block_local::EthLocalBlock;
use edr_chain_spec::{
    BlockEnvChainSpec, BlockEnvConstructor as _, EvmSpecId, ExecutableTransaction,
    TransactionValidation,
};
use edr_chain_spec_block::BlockChainSpec;
use edr_chain_spec_evm::{
    config::EvmConfig, ContextForChainSpec, DatabaseComponentError, EvmChainSpec,
    ExecutionResultAndState, Inspector, TransactionError,
};
use edr_chain_spec_receipt::ReceiptConstructor;
use edr_evm::{dry_run, dry_run_with_inspector};
use edr_primitives::{Address, Bloom, HashMap, KECCAK_NULL_RLP, U256};
use edr_receipt::{
    log::{ExecutionLog, FilterLog},
    ExecutionReceipt, ExecutionReceiptChainSpec, MapReceiptLogs, ReceiptTrait, TransactionReceipt,
};
use edr_receipt_builder_api::ExecutionReceiptBuilder;
use edr_state_api::{AccountModifierFn, DynState, StateDiff, StateError};
use edr_trie::ordered_trie_root;

const MAX_BLOCK_SIZE: usize = 10_485_760; // 10 MiB
const SAFETY_MARGIN: usize = 2_097_152; // 2 MiB

/// EIP-7934 max RLP block size
pub const MAX_RLP_BLOCK_SIZE: usize = MAX_BLOCK_SIZE - SAFETY_MARGIN;

/// A builder for constructing Ethereum blocks.
pub struct EthBlockBuilder<
    'builder,
    BlockReceiptT,
    BlockT: ?Sized,
    BlockchainErrorT: Debug,
    EvmChainSpecT: EvmChainSpec,
    ExecutionReceiptBuilderT: ExecutionReceiptBuilder<
        EvmChainSpecT::HaltReason,
        EvmChainSpecT::Hardfork,
        EvmChainSpecT::SignedTransaction,
        Receipt = ExecutionReceiptChainSpecT::ExecutionReceipt<ExecutionLog>,
    >,
    ExecutionReceiptChainSpecT: ExecutionReceiptChainSpec,
    LocalBlockT,
> {
    blockchain: &'builder dyn Blockchain<
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        EvmChainSpecT::Hardfork,
        LocalBlockT,
        EvmChainSpecT::SignedTransaction,
    >,
    cfg: CfgEnv<EvmChainSpecT::Hardfork>,
    context: EvmChainSpecT::Context,
    header: PartialHeader,
    parent_gas_limit: Option<u64>,
    receipts: Vec<TransactionReceipt<ExecutionReceiptChainSpecT::ExecutionReceipt<ExecutionLog>>>,
    state: Box<dyn DynState>,
    state_diff: StateDiff,
    transactions: Vec<EvmChainSpecT::SignedTransaction>,
    transaction_results: Vec<ExecutionResult<EvmChainSpecT::HaltReason>>,
    withdrawals: Option<Vec<Withdrawal>>,
    custom_precompiles: &'builder HashMap<Address, PrecompileFn>,
    _phantom: PhantomData<fn() -> (EvmChainSpecT, ExecutionReceiptBuilderT)>,
}

impl<
        BlockReceiptT,
        BlockT: ?Sized,
        BlockchainErrorT: Debug,
        EvmChainSpecT: EvmChainSpec<SignedTransaction: ExecutableTransaction>,
        ExecutionReceiptBuilderT: ExecutionReceiptBuilder<
            EvmChainSpecT::HaltReason,
            EvmChainSpecT::Hardfork,
            EvmChainSpecT::SignedTransaction,
            Receipt = ExecutionReceiptChainSpecT::ExecutionReceipt<ExecutionLog>,
        >,
        ExecutionReceiptChainSpecT: ExecutionReceiptChainSpec,
        LocalBlockT,
    >
    EthBlockBuilder<
        '_,
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        EvmChainSpecT,
        ExecutionReceiptBuilderT,
        ExecutionReceiptChainSpecT,
        LocalBlockT,
    >
{
    /// Retrieves the blockchain of the block builder.
    pub fn blockchain(
        &self,
    ) -> &dyn Blockchain<
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        EvmChainSpecT::Hardfork,
        LocalBlockT,
        EvmChainSpecT::SignedTransaction,
    > {
        self.blockchain
    }

    /// Retrieves the config of the block builder.
    pub fn config(&self) -> &CfgEnv<EvmChainSpecT::Hardfork> {
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
    pub fn state(&self) -> &dyn DynState {
        self.state.as_ref()
    }
}

impl<
        BlockReceiptT,
        BlockT: ?Sized,
        BlockchainErrorT: Debug,
        EvmChainSpecT: EvmChainSpec<SignedTransaction: ExecutableTransaction>,
        ExecutionReceiptBuilderT: ExecutionReceiptBuilder<
            EvmChainSpecT::HaltReason,
            EvmChainSpecT::Hardfork,
            EvmChainSpecT::SignedTransaction,
            Receipt = ExecutionReceiptChainSpecT::ExecutionReceipt<ExecutionLog>,
        >,
        ExecutionReceiptChainSpecT: ExecutionReceiptChainSpec,
        LocalBlockT,
    >
    EthBlockBuilder<
        '_,
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        EvmChainSpecT,
        ExecutionReceiptBuilderT,
        ExecutionReceiptChainSpecT,
        LocalBlockT,
    >
{
    fn validate_transaction(
        &self,
        transaction: &EvmChainSpecT::SignedTransaction,
    ) -> Result<
        (),
        BlockTransactionErrorForChainSpec<
            EvmChainSpecT,
            DatabaseComponentError<BlockchainErrorT, StateError>,
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
            let blob_params = blob_params_for_hardfork(self.config().spec.into());

            if block_blob_gas_used + blob_gas_used > blob_params.max_blob_gas_per_block() {
                return Err(BlockTransactionError::ExceedsBlockBlobGasLimit);
            }
        }

        Ok(())
    }
}

impl<
        'builder,
        BlockReceiptT: ReceiptConstructor<
                ChainSpecT::SignedTransaction,
                Context = ChainSpecT::Context,
                ExecutionReceipt = ExecutionReceiptChainSpecT::ExecutionReceipt<FilterLog>,
                Hardfork = ChainSpecT::Hardfork,
            > + ReceiptTrait,
        BlockT: ?Sized + Block<ChainSpecT::SignedTransaction>,
        BlockchainErrorT: Debug + std::error::Error,
        ChainSpecT: BlockChainSpec<Hardfork: PartialOrd, SignedTransaction: Clone + ExecutableTransaction>,
        ExecutionReceiptBuilderT: ExecutionReceiptBuilder<
            ChainSpecT::HaltReason,
            ChainSpecT::Hardfork,
            ChainSpecT::SignedTransaction,
            Receipt = ExecutionReceiptChainSpecT::ExecutionReceipt<ExecutionLog>,
        >,
        ExecutionReceiptChainSpecT: ExecutionReceiptChainSpec<
            ExecutionReceipt<ExecutionLog>: MapReceiptLogs<
                ExecutionLog,
                FilterLog,
                ExecutionReceiptChainSpecT::ExecutionReceipt<FilterLog>,
            > + alloy_rlp::Encodable,
        >,
        LocalBlockT: From<
            EthLocalBlock<
                BlockReceiptT,
                ChainSpecT::FetchReceiptError,
                ChainSpecT::Hardfork,
                ChainSpecT::SignedTransaction,
            >,
        >,
    >
    EthBlockBuilder<
        'builder,
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        ChainSpecT,
        ExecutionReceiptBuilderT,
        ExecutionReceiptChainSpecT,
        LocalBlockT,
    >
{
    /// Creates a new instance.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn new(
        context: ChainSpecT::Context,
        blockchain: &'builder dyn Blockchain<
            BlockReceiptT,
            BlockT,
            BlockchainErrorT,
            ChainSpecT::Hardfork,
            LocalBlockT,
            ChainSpecT::SignedTransaction,
        >,
        state: Box<dyn DynState>,
        evm_config: &EvmConfig,
        inputs: BlockInputs,
        mut overrides: HeaderOverrides<ChainSpecT::Hardfork>,
        custom_precompiles: &'builder HashMap<Address, PrecompileFn>,
    ) -> Result<
        Self,
        BlockBuilderCreationError<
            DatabaseComponentError<BlockchainErrorT, StateError>,
            ChainSpecT::Hardfork,
        >,
    > {
        let parent_block = blockchain.last_block().map_err(|error| {
            BlockBuilderCreationError::Database(DatabaseComponentError::Blockchain(error))
        })?;

        let hardfork = blockchain.hardfork();

        let evm_spec_id = hardfork.into();
        if evm_spec_id < EvmSpecId::BYZANTIUM {
            return Err(BlockBuilderCreationError::UnsupportedHardfork(hardfork));
        } else if evm_spec_id >= EvmSpecId::SHANGHAI && inputs.withdrawals.is_none() {
            return Err(BlockBuilderCreationError::MissingWithdrawals);
        }

        let parent_header = parent_block.block_header();
        let parent_gas_limit = if overrides.gas_limit.is_none() {
            Some(parent_header.gas_limit)
        } else {
            None
        };

        overrides.parent_hash = Some(*parent_block.block_hash());

        let cfg = evm_config.to_cfg_env(hardfork);
        let header = PartialHeader::new(
            BlockConfig {
                base_fee_params: blockchain.base_fee_params(),
                hardfork,
                min_ethash_difficulty: blockchain.min_ethash_difficulty(),
            },
            overrides,
            Some(parent_header),
            &inputs.ommers,
            inputs.withdrawals.as_ref(),
        );

        Ok(Self {
            blockchain,
            cfg,
            context,
            header,
            parent_gas_limit,
            receipts: Vec::new(),
            state,
            state_diff: StateDiff::default(),
            transactions: Vec::new(),
            transaction_results: Vec::new(),
            withdrawals: inputs.withdrawals,
            custom_precompiles,
            _phantom: PhantomData,
        })
    }

    /// Tries to add a transaction to the block.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn add_transaction(
        &mut self,
        transaction: ChainSpecT::SignedTransaction,
    ) -> Result<
        (),
        BlockTransactionErrorForChainSpec<
            ChainSpecT,
            DatabaseComponentError<BlockchainErrorT, StateError>,
        >,
    > {
        self.validate_transaction(&transaction)?;

        let block_env = HeaderAndEvmSpec {
            header: &self.header,
            hardfork: self.cfg.spec.into(),
        };

        let receipt_builder =
            ExecutionReceiptBuilderT::new_receipt_builder(&self.state, &transaction).map_err(
                |error| {
                    BlockTransactionError::Transaction(TransactionError::Database(
                        DatabaseComponentError::State(error),
                    ))
                },
            )?;

        let transaction_result = dry_run::<ChainSpecT, _, _, _>(
            self.blockchain,
            &self.state,
            self.cfg.clone(),
            transaction.clone(),
            block_env,
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
    ) -> Result<
        (),
        BlockTransactionErrorForChainSpec<
            ChainSpecT,
            DatabaseComponentError<BlockchainErrorT, StateError>,
        >,
    >
    where
        InspectorT: for<'inspector> Inspector<
            ContextForChainSpec<
                ChainSpecT,
                ChainSpecT::BlockEnv<'inspector, PartialHeader>,
                WrapDatabaseRef<
                    DatabaseComponents<
                        &'inspector dyn Blockchain<
                            BlockReceiptT,
                            BlockT,
                            BlockchainErrorT,
                            ChainSpecT::Hardfork,
                            LocalBlockT,
                            ChainSpecT::SignedTransaction,
                        >,
                        &'inspector dyn DynState,
                    >,
                >,
            >,
        >,
    {
        self.validate_transaction(&transaction)?;

        let block_env = ChainSpecT::BlockEnv::new_block_env(&self.header, self.cfg.spec);

        let receipt_builder =
            ExecutionReceiptBuilderT::new_receipt_builder(&self.state, &transaction).map_err(
                |error| {
                    BlockTransactionError::Transaction(TransactionError::Database(
                        DatabaseComponentError::State(error),
                    ))
                },
            )?;

        let transaction_result = dry_run_with_inspector::<ChainSpecT, _, _, _, _>(
            self.blockchain,
            self.state.as_ref(),
            self.cfg.clone(),
            transaction.clone(),
            block_env,
            self.custom_precompiles,
            extension,
        )
        .map_err(BlockTransactionError::from)?;

        self.add_transaction_result(receipt_builder, transaction, transaction_result);

        Ok(())
    }
    fn add_transaction_result(
        &mut self,
        receipt_builder: ExecutionReceiptBuilderT,
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

impl<
        'builder,
        BlockReceiptT: ReceiptConstructor<
                ChainSpecT::SignedTransaction,
                Context = ChainSpecT::Context,
                ExecutionReceipt = ExecutionReceiptChainSpecT::ExecutionReceipt<FilterLog>,
                Hardfork = ChainSpecT::Hardfork,
            > + ReceiptTrait
            + alloy_rlp::Encodable,
        BlockT: ?Sized + Block<ChainSpecT::SignedTransaction>,
        BlockchainErrorT: Debug + std::error::Error,
        ChainSpecT: BlockChainSpec<
            Hardfork: PartialOrd,
            SignedTransaction: Clone + ExecutableTransaction + alloy_rlp::Encodable,
        >,
        ExecutionReceiptBuilderT: ExecutionReceiptBuilder<
            ChainSpecT::HaltReason,
            ChainSpecT::Hardfork,
            ChainSpecT::SignedTransaction,
            Receipt = ExecutionReceiptChainSpecT::ExecutionReceipt<ExecutionLog>,
        >,
        ExecutionReceiptChainSpecT: ExecutionReceiptChainSpec<
            ExecutionReceipt<ExecutionLog>: MapReceiptLogs<
                ExecutionLog,
                FilterLog,
                ExecutionReceiptChainSpecT::ExecutionReceipt<FilterLog>,
            > + alloy_rlp::Encodable,
        >,
        LocalBlockT: From<
            EthLocalBlock<
                BlockReceiptT,
                ChainSpecT::FetchReceiptError,
                ChainSpecT::Hardfork,
                ChainSpecT::SignedTransaction,
            >,
        >,
    >
    EthBlockBuilder<
        'builder,
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        ChainSpecT,
        ExecutionReceiptBuilderT,
        ExecutionReceiptChainSpecT,
        LocalBlockT,
    >
{
    pub fn finalize(
        mut self,
        rewards: Vec<(Address, u128)>,
    ) -> Result<
        BuiltBlockAndState<ChainSpecT::HaltReason, LocalBlockT>,
        BlockFinalizeError<StateError>,
    > {
        for (address, reward) in rewards {
            if reward > 0 {
                let account_info = self
                    .state
                    .modify_account(
                        address,
                        AccountModifierFn::new(Box::new(move |balance, _nonce, _code| {
                            *balance += U256::from(reward);
                        })),
                    )
                    .map_err(BlockFinalizeError::State)?;

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
        let block = EthLocalBlock::new::<ExecutionReceiptChainSpecT>(
            &self.context,
            self.cfg.spec,
            self.header,
            self.transactions,
            self.receipts,
            Vec::new(),
            self.withdrawals,
        );

        let block_rlp_size = alloy_rlp::Encodable::length(&block);
        if block_rlp_size > MAX_RLP_BLOCK_SIZE {
            return Err(BlockFinalizeError::BlockRlpSizeExceeded {
                max_size: MAX_RLP_BLOCK_SIZE,
                actual_size: block_rlp_size,
            });
        }

        Ok(BuiltBlockAndState {
            block: block.into(),
            state: self.state,
            state_diff: self.state_diff,
            transaction_results: self.transaction_results,
        })
    }
}

impl<
        'builder,
        BlockReceiptT: ReceiptConstructor<
                ChainSpecT::SignedTransaction,
                Context = ChainSpecT::Context,
                ExecutionReceipt = ExecutionReceiptChainSpecT::ExecutionReceipt<FilterLog>,
                Hardfork = ChainSpecT::Hardfork,
            > + ReceiptTrait
            + alloy_rlp::Encodable,
        BlockT: ?Sized + Block<ChainSpecT::SignedTransaction>,
        BlockchainErrorT: Debug + std::error::Error,
        ChainSpecT: BlockChainSpec
            + BlockEnvChainSpec
            + EvmChainSpec<
                Context: Default,
                Hardfork: PartialOrd,
                SignedTransaction: Clone + ExecutableTransaction + alloy_rlp::Encodable,
            >,
        ExecutionReceiptBuilderT: ExecutionReceiptBuilder<
            ChainSpecT::HaltReason,
            ChainSpecT::Hardfork,
            ChainSpecT::SignedTransaction,
            Receipt = ExecutionReceiptChainSpecT::ExecutionReceipt<ExecutionLog>,
        >,
        ExecutionReceiptChainSpecT: ExecutionReceiptChainSpec<
            ExecutionReceipt<ExecutionLog>: MapReceiptLogs<
                ExecutionLog,
                FilterLog,
                ExecutionReceiptChainSpecT::ExecutionReceipt<FilterLog>,
            > + alloy_rlp::Encodable,
        >,
        LocalBlockT: From<
            EthLocalBlock<
                BlockReceiptT,
                ChainSpecT::FetchReceiptError,
                ChainSpecT::Hardfork,
                ChainSpecT::SignedTransaction,
            >,
        >,
    > BlockBuilder<'builder, ChainSpecT, BlockReceiptT, BlockT>
    for EthBlockBuilder<
        'builder,
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        ChainSpecT,
        ExecutionReceiptBuilderT,
        ExecutionReceiptChainSpecT,
        LocalBlockT,
    >
{
    type BlockchainError = BlockchainErrorT;

    type LocalBlock = LocalBlockT;

    fn new_block_builder(
        blockchain: &'builder dyn Blockchain<
            BlockReceiptT,
            BlockT,
            Self::BlockchainError,
            ChainSpecT::Hardfork,
            LocalBlockT,
            ChainSpecT::SignedTransaction,
        >,
        state: Box<dyn DynState>,
        evm_config: &EvmConfig,
        inputs: BlockInputs,
        overrides: HeaderOverrides<ChainSpecT::Hardfork>,
        custom_precompiles: &'builder HashMap<Address, PrecompileFn>,
    ) -> Result<
        Self,
        BlockBuilderCreationError<
            DatabaseComponentError<Self::BlockchainError, StateError>,
            ChainSpecT::Hardfork,
        >,
    > {
        Self::new(
            ChainSpecT::Context::default(),
            blockchain,
            state,
            evm_config,
            inputs,
            overrides,
            custom_precompiles,
        )
    }

    fn header(&self) -> &PartialHeader {
        self.header()
    }

    fn add_transaction(
        &mut self,
        transaction: ChainSpecT::SignedTransaction,
    ) -> Result<
        (),
        BlockTransactionError<
            DatabaseComponentError<Self::BlockchainError, StateError>,
            <ChainSpecT::SignedTransaction as TransactionValidation>::ValidationError,
        >,
    > {
        Self::add_transaction(self, transaction)
    }

    fn add_transaction_with_inspector<InspectorT>(
        &mut self,
        transaction: ChainSpecT::SignedTransaction,
        inspector: &mut InspectorT,
    ) -> Result<
        (),
        BlockTransactionError<
            DatabaseComponentError<Self::BlockchainError, StateError>,
            <ChainSpecT::SignedTransaction as TransactionValidation>::ValidationError,
        >,
    >
    where
        InspectorT: for<'inspector> Inspector<
            ContextForChainSpec<
                ChainSpecT,
                ChainSpecT::BlockEnv<'inspector, PartialHeader>,
                WrapDatabaseRef<
                    DatabaseComponents<
                        &'inspector dyn Blockchain<
                            BlockReceiptT,
                            BlockT,
                            Self::BlockchainError,
                            ChainSpecT::Hardfork,
                            LocalBlockT,
                            ChainSpecT::SignedTransaction,
                        >,
                        &'inspector dyn DynState,
                    >,
                >,
            >,
        >,
    {
        Self::add_transaction_with_inspector(self, transaction, inspector)
    }

    fn finalize_block(
        self,
        rewards: Vec<(Address, u128)>,
    ) -> Result<
        BuiltBlockAndState<ChainSpecT::HaltReason, LocalBlockT>,
        BlockFinalizeError<StateError>,
    > {
        self.finalize(rewards)
    }
}
