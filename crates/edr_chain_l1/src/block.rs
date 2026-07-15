use core::{fmt::Debug, marker::PhantomData};
use std::time::{SystemTime, UNIX_EPOCH};

use alloy_trie::root::ordered_trie_root;
use edr_block_api::Block;
use edr_block_builder_api::{
    BlockBuilder, BlockBuilderCreationError, BlockFinalizeError, BlockInputs,
    BlockTransactionError, BlockTransactionErrorForChainSpec, Blockchain, BuiltBlockAndState,
    BuiltBlockAndStateWithMetadata, CfgEnv, DatabaseComponents, ExecutionResult, PrecompileFn,
    WrapDatabaseRef,
};
use edr_block_header::{
    blob_params_for_hardfork, BlobGas, BlockConfig, HeaderAndEvmSpec, HeaderOverrides,
    PartialHeader, Withdrawal,
};
use edr_block_local::EthLocalBlock;
use edr_chain_spec::{
    BlockEnvChainSpec, BlockEnvConstructor as _, ChainSpec, EvmSpecId, ExecutableTransaction,
    HardforkChainSpec, TransactionValidation,
};
use edr_chain_spec_block::BlockChainSpec;
use edr_chain_spec_evm::{
    config::EvmConfig, ContextForChainSpec, DatabaseComponentError, EvmChainSpec,
    ExecutionResultAndState, Inspector, TransactionError,
};
use edr_chain_spec_receipt::ReceiptConstructor;
use edr_evm::{dry_run, dry_run_with_inspector};
use edr_precompile::OverriddenPrecompileProvider;
use edr_primitives::{
    keccak256, Address, Bloom, HashMap, HashSet, B256, KECCAK_NULL_RLP, KECCAK_RLP_EMPTY_ARRAY,
    U256,
};
use edr_receipt::{
    log::{ExecutionLog, FilterLog},
    ExecutionReceipt, ExecutionReceiptChainSpec, MapReceiptLogs, ReceiptTrait, TransactionReceipt,
};
use edr_receipt_builder_api::ExecutionReceiptBuilder;
use edr_state_api::{AccountModifierFn, DynState, StateDiff, StateError};

const MAX_BLOCK_SIZE: usize = 10_485_760; // 10 MiB
const SAFETY_MARGIN: usize = 2_097_152; // 2 MiB

/// EIP-7934 max RLP block size
pub const MAX_RLP_BLOCK_SIZE: usize = MAX_BLOCK_SIZE - SAFETY_MARGIN;

/// A builder for constructing Ethereum blocks.
pub struct EthBlockBuilder<
    'builder,
    BlockReceiptT,
    BlockT: ?Sized,
    BlockchainErrorT: Debug + Send + Sync + 'static,
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
    block_config: &'builder BlockConfig<EvmChainSpecT::Hardfork>,
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
    // Set of all unique precompile addresses. We collect this once during construction as their
    // creation should be deterministic.
    precompile_addresses: HashSet<Address>,
    _phantom: PhantomData<fn() -> (EvmChainSpecT, ExecutionReceiptBuilderT)>,
    // Net cumulative gas used (after refunds), tracked separately because from Amsterdam
    // (EIP-7778) the header's `gas_used` is gross (before refunds).
    cumulative_gas_used: u64,
}

impl<
        BlockReceiptT,
        BlockT: ?Sized,
        BlockchainErrorT: Debug + Send + Sync + 'static,
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
        BlockchainErrorT: Debug + Send + Sync + 'static,
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
        // block, unless the block gas limit check is disabled.
        if !self.cfg.disable_block_gas_limit && transaction.gas_limit() > self.gas_remaining() {
            return Err(BlockTransactionError::ExceedsBlockGasLimit);
        }

        let blob_gas_used = transaction.total_blob_gas().unwrap_or_default();
        // Checking `blob_hashes` is a hack for preventing OP stack Jovian block
        // transactions go through this validation since the validation may
        // fail. This is because Jovian repurposes the block header
        // `blobGasUsed` field to store the DA footprint. See <https://specs.optimism.io/protocol/jovian/exec-engine.html#da-footprint-block-limit>
        // TODO: Use a custom OP validator for OP transactions <https://github.com/NomicFoundation/edr/issues/1212>
        if !transaction.blob_hashes().is_empty()
            && let Some(BlobGas {
                gas_used: block_blob_gas_used,
                ..
            }) = self.header.blob_gas.as_ref()
        {
            let blob_params = blob_params_for_hardfork(
                self.config().spec.into(),
                self.header.timestamp,
                self.block_config.scheduled_blob_params.as_ref(),
            );

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
        BlockchainErrorT: Debug + 'static + std::error::Error + Send + Sync,
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
    #[allow(clippy::too_many_arguments)]
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
        block_config: &'builder BlockConfig<ChainSpecT::Hardfork>,
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
            block_config,
            overrides,
            Some(parent_header),
            &inputs.ommers,
            inputs.withdrawals.as_ref(),
        );

        let precompile_addresses = {
            #[allow(clippy::type_complexity)]
            let precompile_provider: OverriddenPrecompileProvider<
                _,
                ContextForChainSpec<
                    ChainSpecT,
                    HeaderAndEvmSpec<'builder, PartialHeader, ChainSpecT::Hardfork>,
                    WrapDatabaseRef<
                        DatabaseComponents<
                            &'builder dyn Blockchain<
                                BlockReceiptT,
                                BlockT,
                                BlockchainErrorT,
                                ChainSpecT::Hardfork,
                                LocalBlockT,
                                ChainSpecT::SignedTransaction,
                            >,
                            &'builder dyn DynState,
                        >,
                    >,
                >,
            > = OverriddenPrecompileProvider::with_precompiles(
                ChainSpecT::new_precompile_provider(hardfork),
                custom_precompiles.clone(),
            );
            precompile_provider.into_addresses()
        };

        Ok(Self {
            blockchain,
            block_config,
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
            precompile_addresses,
            _phantom: PhantomData,
            cumulative_gas_used: 0,
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

        let block_env = HeaderAndEvmSpec::new_block_env(
            &self.header,
            self.cfg.spec.into(),
            self.block_config.scheduled_blob_params.as_ref(),
        );

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

        self.add_transaction_result(
            receipt_builder,
            transaction,
            transaction_result.into_result_and_state(),
        );

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

        let block_env = ChainSpecT::BlockEnv::new_block_env(
            &self.header,
            self.cfg.spec,
            self.block_config.scheduled_blob_params.as_ref(),
        );

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

        self.add_transaction_result(
            receipt_builder,
            transaction,
            transaction_result.into_result_and_state(),
        );

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

        self.cumulative_gas_used += transaction_result.tx_gas_used();
        self.header.gas_used +=
            transaction_block_gas_contribution::<ChainSpecT>(self.cfg.spec, &transaction_result);

        if let Some(BlobGas { gas_used, .. }) = self.header.blob_gas.as_mut() {
            let blob_gas_used = transaction.total_blob_gas().unwrap_or_default();
            *gas_used += blob_gas_used;
        }

        let receipt = receipt_builder.build_receipt(
            &transaction,
            &transaction_result,
            self.cfg.spec,
            self.cumulative_gas_used,
            self.header.state_root,
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

/// Gas a transaction contributes to the block's `gas_used`: from Amsterdam
/// (EIP-7778) the gross gas before refunds, otherwise its net gas used.
fn transaction_block_gas_contribution<ChainSpecT: ChainSpec + HardforkChainSpec>(
    hardfork: ChainSpecT::Hardfork,
    execution_result: &ExecutionResult<ChainSpecT::HaltReason>,
) -> u64 {
    if hardfork.into() >= EvmSpecId::AMSTERDAM {
        let execution_gas = execution_result.gas();
        execution_gas
            .total_gas_spent()
            .max(execution_gas.floor_gas())
    } else {
        execution_result.tx_gas_used()
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
        BlockchainErrorT: Debug + 'static + std::error::Error + Send + Sync,
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
        BuiltBlockAndStateWithMetadata<LocalBlockT, ChainSpecT::HaltReason>,
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

        self.header.receipts_root = ordered_trie_root(&self.receipts);

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

        // Must run after the reward loop and state-root computation above, so
        // `state_diff` is final.
        self.header.block_access_list_hash = block_access_list_hash(
            self.header.block_access_list_hash,
            &self.state_diff,
            self.header.parent_hash,
        );

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

        Ok(BuiltBlockAndStateWithMetadata {
            block_and_state: BuiltBlockAndState {
                block: block.into(),
                state: self.state,
                state_diff: self.state_diff,
                transaction_results: self.transaction_results,
            },
            precompile_addresses: self.precompile_addresses,
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
        BlockchainErrorT: Debug + 'static + std::error::Error + Send + Sync,
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
        block_config: &'builder BlockConfig<ChainSpecT::Hardfork>,
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
            block_config,
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

    fn precompile_addresses(&self) -> &HashSet<Address> {
        &self.precompile_addresses
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
        BuiltBlockAndStateWithMetadata<LocalBlockT, ChainSpecT::HaltReason>,
        BlockFinalizeError<StateError>,
    > {
        self.finalize(rewards)
    }
}

/// Resolves a block's simulated block access list hash (EIP-7928), upholding
/// two guarantees:
/// - a block that introduces no state changes keeps the empty-RLP-list hash, as
///   the EIP specifies;
/// - no two blocks in the same chain share a hash: a state-changing block's
///   hash is derived from its parent hash, which is unique per block.
///   (Reverting to a snapshot forks the chain, so the same hash can reoccur on
///   that fork — but not within a single chain.)
///
/// `current` is the value the header already carries (the empty-list default,
/// or a hash supplied externally); only the empty-list default of a
/// state-changing block is replaced.
fn block_access_list_hash(
    current: Option<B256>,
    state_diff: &StateDiff,
    parent_hash: B256,
) -> Option<B256> {
    if current == Some(KECCAK_RLP_EMPTY_ARRAY) && !state_diff.as_inner().is_empty() {
        Some(keccak256(
            format!("blockAccessListHash{parent_hash}").as_bytes(),
        ))
    } else {
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    mod block_access_list {
        use edr_state_api::account::AccountInfo;

        use super::*;
        fn non_empty_state_diff() -> StateDiff {
            let mut state_diff = StateDiff::default();
            state_diff.apply_account_change(Address::ZERO, AccountInfo::default());
            state_diff
        }

        const PARENT_HASH: B256 = B256::repeat_byte(9);

        #[test]
        fn keeps_absent_hash() {
            // A header without the field keeps it absent, whatever the state.
            assert_eq!(
                block_access_list_hash(None, &non_empty_state_diff(), PARENT_HASH),
                None
            );
        }

        #[test]
        fn keeps_empty_list_hash_when_state_unchanged() {
            assert_eq!(
                block_access_list_hash(
                    Some(KECCAK_RLP_EMPTY_ARRAY),
                    &StateDiff::default(),
                    PARENT_HASH
                ),
                Some(KECCAK_RLP_EMPTY_ARRAY)
            );
        }

        #[test]
        fn keeps_externally_supplied_hash() {
            let supplied = B256::repeat_byte(0xab);
            assert_eq!(
                block_access_list_hash(Some(supplied), &non_empty_state_diff(), PARENT_HASH),
                Some(supplied)
            );
        }

        #[test]
        fn upgrades_empty_list_hash_when_state_changed() {
            let hash = block_access_list_hash(
                Some(KECCAK_RLP_EMPTY_ARRAY),
                &non_empty_state_diff(),
                PARENT_HASH,
            )
            .expect("should produce a hash");

            // Upgraded away from the empty-list default, and reproducible for the same
            // inputs.
            assert_ne!(hash, KECCAK_RLP_EMPTY_ARRAY);
            assert_eq!(
                block_access_list_hash(
                    Some(KECCAK_RLP_EMPTY_ARRAY),
                    &non_empty_state_diff(),
                    PARENT_HASH
                ),
                Some(hash)
            );
        }

        #[test]
        fn upgraded_hash_varies_with_parent_hash() {
            // Distinct per block: the parent hash is unique per block within a chain, so no
            // two sibling blocks share a hash (including in forked mode).
            assert_ne!(
                block_access_list_hash(
                    Some(KECCAK_RLP_EMPTY_ARRAY),
                    &non_empty_state_diff(),
                    B256::repeat_byte(2)
                ),
                block_access_list_hash(
                    Some(KECCAK_RLP_EMPTY_ARRAY),
                    &non_empty_state_diff(),
                    B256::repeat_byte(3)
                ),
            );
        }
    }

    mod transaction_block_gas_contribution {
        use edr_chain_spec_evm::result::{Output, ResultGas, SuccessReason};
        use edr_primitives::Bytes;

        use super::*;
        use crate::{HaltReason, Hardfork, L1ChainSpec};

        // A successful result carrying the given raw gas figures.
        fn execution_result(
            total_gas_spent: u64,
            refunded: u64,
            floor_gas: u64,
        ) -> ExecutionResult<HaltReason> {
            ExecutionResult::Success {
                reason: SuccessReason::Stop,
                gas: ResultGas::default()
                    .with_total_gas_spent(total_gas_spent)
                    .with_refunded(refunded)
                    .with_floor_gas(floor_gas),
                logs: Vec::new(),
                output: Output::Call(Bytes::new()),
            }
        }

        #[test]
        fn before_amsterdam_uses_gas_after_refunds() {
            // Net gas: total - refund = 40_000, above the (30_000) floor.
            let result = execution_result(50_000, 10_000, 30_000);
            assert_eq!(
                transaction_block_gas_contribution::<L1ChainSpec>(Hardfork::OSAKA, &result),
                40_000
            );
        }

        #[test]
        fn from_amsterdam_uses_gas_before_refunds() {
            // EIP-7778: the refund is not subtracted from the block gas.
            let result = execution_result(50_000, 10_000, 0);
            assert_eq!(
                transaction_block_gas_contribution::<L1ChainSpec>(Hardfork::AMSTERDAM, &result),
                50_000
            );
        }

        #[test]
        fn from_amsterdam_applies_calldata_floor() {
            // The EIP-7623 floor still applies when it exceeds the gas spent.
            let result = execution_result(20_000, 0, 25_000);
            assert_eq!(
                transaction_block_gas_contribution::<L1ChainSpec>(Hardfork::AMSTERDAM, &result),
                25_000
            );
        }

        #[test]
        fn before_amsterdam_applies_calldata_floor() {
            // Net gas is floored too: max(total - refund, floor).
            let result = execution_result(50_000, 10_000, 45_000);
            assert_eq!(
                transaction_block_gas_contribution::<L1ChainSpec>(Hardfork::OSAKA, &result),
                45_000
            );
        }
    }
}
