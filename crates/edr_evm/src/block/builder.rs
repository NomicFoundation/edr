use std::{
    fmt::Debug,
    time::{SystemTime, UNIX_EPOCH},
};

use edr_eth::{
    block::{BlobGas, BlockOptions, PartialHeader},
    log::ExecutionLog,
    receipt::{ExecutionReceiptBuilder as _, Receipt as _, TransactionReceipt},
    result::InvalidTransaction,
    transaction::SignedTransaction as _,
    trie::{ordered_trie_root, KECCAK_NULL_RLP},
    withdrawal::Withdrawal,
    Address, Bloom, U256,
};
use revm::{
    db::{DatabaseComponents, StateRef},
    handler::{CfgEnvWithChainSpec, EnvWithChainSpec},
    primitives::{
        ExecutionResult, ResultAndState, SpecId, Transaction as _, TransactionValidation,
        MAX_BLOB_GAS_PER_BLOCK,
    },
    Context, DatabaseCommit, Evm, InnerEvmContext,
};

use super::local::LocalBlock;
use crate::{
    blockchain::SyncBlockchain,
    chain_spec::{BlockEnvConstructor, ChainSpec},
    debug::{DebugContext, EvmContext},
    state::{AccountModifierFn, StateDebug, StateDiff, SyncState},
    transaction::TransactionError,
    SyncBlock,
};

/// An error caused during construction of a block builder.
#[derive(Debug, thiserror::Error)]
pub enum BlockBuilderCreationError<ChainSpecT>
where
    ChainSpecT: ChainSpec<Hardfork: Debug>,
{
    /// Unsupported hardfork. Hardforks older than Byzantium are not supported
    #[error("Unsupported hardfork: {0:?}. Hardforks older than Byzantium are not supported.")]
    UnsupportedHardfork(ChainSpecT::Hardfork),
}

/// An error caused during execution of a transaction while building a block.
#[derive(Debug, thiserror::Error)]
pub enum BlockTransactionError<ChainSpecT, BlockchainErrorT, StateErrorT>
where
    ChainSpecT: revm::primitives::ChainSpec,
{
    /// Transaction has higher gas limit than is remaining in block
    #[error("Transaction has a higher gas limit than the remaining gas in the block")]
    ExceedsBlockGasLimit,
    /// Transaction has higher blob gas usage than is remaining in block
    #[error("Transaction has higher blob gas usage than is remaining in block")]
    ExceedsBlockBlobGasLimit,
    /// Transaction error
    #[error(transparent)]
    Transaction(#[from] TransactionError<ChainSpecT, BlockchainErrorT, StateErrorT>),
}

/// The result of executing a transaction, along with the context in which it
/// was executed.
pub struct ExecutionResultWithContext<
    'evm,
    ChainSpecT,
    BlockchainErrorT,
    StateErrorT,
    DebugDataT,
    StateT: StateRef,
> where
    ChainSpecT: revm::ChainSpec,
{
    /// The result of executing the transaction.
    pub result: Result<
        ExecutionResult<ChainSpecT>,
        BlockTransactionError<ChainSpecT, BlockchainErrorT, StateErrorT>,
    >,
    /// The context in which the transaction was executed.
    pub evm_context: EvmContext<'evm, ChainSpecT, BlockchainErrorT, DebugDataT, StateT>,
}

/// The result of building a block, using the [`BlockBuilder`].
pub struct BuildBlockResult<ChainSpecT: ChainSpec> {
    /// Built block
    pub block: LocalBlock<ChainSpecT>,
    /// State diff
    pub state_diff: StateDiff,
}

/// A builder for constructing Ethereum blocks.
pub struct BlockBuilder<ChainSpecT: ChainSpec> {
    cfg: CfgEnvWithChainSpec<ChainSpecT>,
    header: PartialHeader,
    transactions: Vec<ChainSpecT::Transaction>,
    state_diff: StateDiff,
    receipts: Vec<TransactionReceipt<ChainSpecT::ExecutionReceipt<ExecutionLog>, ExecutionLog>>,
    parent_gas_limit: Option<u64>,
    withdrawals: Option<Vec<Withdrawal>>,
}

impl<ChainSpecT> BlockBuilder<ChainSpecT>
where
    ChainSpecT: ChainSpec<Hardfork: Debug>,
{
    /// Creates an intance of [`BlockBuilder`].
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn new<BlockchainErrorT>(
        cfg: CfgEnvWithChainSpec<ChainSpecT>,
        parent: &dyn SyncBlock<ChainSpecT, Error = BlockchainErrorT>,
        mut options: BlockOptions,
    ) -> Result<Self, BlockBuilderCreationError<ChainSpecT>> {
        if cfg.spec_id.into() < SpecId::BYZANTIUM {
            return Err(BlockBuilderCreationError::UnsupportedHardfork(cfg.spec_id));
        }

        let parent_header = parent.header();
        let parent_gas_limit = if options.gas_limit.is_none() {
            Some(parent_header.gas_limit)
        } else {
            None
        };

        let withdrawals = std::mem::take(&mut options.withdrawals).or_else(|| {
            if cfg.spec_id.into() >= SpecId::SHANGHAI {
                Some(Vec::new())
            } else {
                None
            }
        });

        options.parent_hash = Some(*parent.hash());
        let header = PartialHeader::new::<ChainSpecT>(cfg.spec_id, options, Some(parent_header));

        Ok(Self {
            cfg,
            header,
            transactions: Vec::new(),
            state_diff: StateDiff::default(),
            receipts: Vec::new(),
            parent_gas_limit,
            withdrawals,
        })
    }
}

impl<ChainSpecT: ChainSpec> BlockBuilder<ChainSpecT> {
    /// Retrieves the config of the block builder.
    pub fn config(&self) -> &CfgEnvWithChainSpec<ChainSpecT> {
        &self.cfg
    }

    /// Retrieves the amount of gas used in the block, so far.
    pub fn gas_used(&self) -> u64 {
        self.header.gas_used
    }

    /// Retrieves the amount of gas left in the block.
    pub fn gas_remaining(&self) -> u64 {
        self.header.gas_limit - self.gas_used()
    }

    /// Retrieves the header of the block builder.
    pub fn header(&self) -> &PartialHeader {
        &self.header
    }

    /// Finalizes the block, returning the block and the callers of the
    /// transactions.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn finalize<StateT, StateErrorT>(
        mut self,
        state: &mut StateT,
        rewards: Vec<(Address, U256)>,
    ) -> Result<BuildBlockResult<ChainSpecT>, StateErrorT>
    where
        StateT: SyncState<StateErrorT> + ?Sized,
        StateErrorT: Debug + Send,
    {
        for (address, reward) in rewards {
            if reward > U256::ZERO {
                let account_info = state.modify_account(
                    address,
                    AccountModifierFn::new(Box::new(move |balance, _nonce, _code| {
                        *balance += reward;
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
            self.header.state_root = state
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
        let block = LocalBlock::new(
            self.header,
            self.transactions,
            self.receipts,
            Vec::new(),
            self.withdrawals,
        );

        Ok(BuildBlockResult {
            block,
            state_diff: self.state_diff,
        })
    }
}

impl<ChainSpecT> BlockBuilder<ChainSpecT>
where
    ChainSpecT: ChainSpec<
        Block: Default,
        Transaction: Clone
                         + Default
                         + TransactionValidation<ValidationError: From<InvalidTransaction>>,
    >,
{
    /// Adds a pending transaction to
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn add_transaction<'blockchain, 'evm, BlockchainErrorT, DebugDataT, StateT, StateErrorT>(
        &mut self,
        blockchain: &'blockchain dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateErrorT>,
        state: StateT,
        transaction: ChainSpecT::Transaction,
        debug_context: Option<DebugContext<'evm, ChainSpecT, BlockchainErrorT, DebugDataT, StateT>>,
    ) -> ExecutionResultWithContext<
        'evm,
        ChainSpecT,
        BlockchainErrorT,
        StateErrorT,
        DebugDataT,
        StateT,
    >
    where
        'blockchain: 'evm,
        BlockchainErrorT: Debug + Send,
        StateT: StateRef<Error = StateErrorT> + DatabaseCommit + StateDebug<Error = StateErrorT>,
        StateErrorT: Debug + Send,
    {
        // The transaction's gas limit cannot be greater than the remaining gas in the
        // block
        if transaction.gas_limit() > self.gas_remaining() {
            return ExecutionResultWithContext {
                result: Err(BlockTransactionError::ExceedsBlockGasLimit),
                evm_context: EvmContext {
                    debug: debug_context,
                    state,
                },
            };
        }

        let blob_gas_used = transaction.total_blob_gas().unwrap_or_default();
        if let Some(BlobGas {
            gas_used: block_blob_gas_used,
            ..
        }) = self.header.blob_gas.as_ref()
        {
            if block_blob_gas_used + blob_gas_used > MAX_BLOB_GAS_PER_BLOCK {
                return ExecutionResultWithContext {
                    result: Err(BlockTransactionError::ExceedsBlockBlobGasLimit),
                    evm_context: EvmContext {
                        debug: debug_context,
                        state,
                    },
                };
            }
        }

        let spec_id = self.cfg.spec_id;
        let block = ChainSpecT::Block::new_block_env(&self.header, spec_id);

        let receipt_builder = {
            let builder = ChainSpecT::ReceiptBuilder::new_receipt_builder(&state, &transaction);

            match builder {
                Ok(builder) => builder,
                Err(error) => {
                    return ExecutionResultWithContext {
                        result: Err(TransactionError::State(error).into()),
                        evm_context: EvmContext {
                            debug: debug_context,
                            state,
                        },
                    };
                }
            }
        };

        let env = EnvWithChainSpec::new_with_cfg_env(self.cfg.clone(), block, transaction.clone());

        let db = DatabaseComponents {
            state,
            block_hash: blockchain,
        };

        let (
            mut evm_context,
            ResultAndState {
                result,
                state: state_diff,
            },
        ) = {
            if let Some(debug_context) = debug_context {
                let mut evm = Evm::builder()
                    .with_chain_spec::<ChainSpecT>()
                    .with_ref_db(db)
                    .with_external_context(debug_context.data)
                    .with_env_with_handler_cfg(env)
                    .append_handler_register(debug_context.register_handles_fn)
                    .build();

                let result = evm.transact();
                let Context {
                    evm:
                        revm::EvmContext {
                            inner: InnerEvmContext { db, .. },
                            ..
                        },
                    external,
                } = evm.into_context();

                let evm_context = EvmContext {
                    debug: Some(DebugContext {
                        data: external,
                        register_handles_fn: debug_context.register_handles_fn,
                    }),
                    state: db.0.state,
                };

                match result {
                    Ok(result) => (evm_context, result),
                    Err(error) => {
                        return ExecutionResultWithContext {
                            result: Err(TransactionError::from(error).into()),
                            evm_context,
                        };
                    }
                }
            } else {
                let mut evm = Evm::builder()
                    .with_chain_spec::<ChainSpecT>()
                    .with_ref_db(db)
                    .with_env_with_handler_cfg(env)
                    .build();

                let result = evm.transact();
                let Context {
                    evm:
                        revm::EvmContext {
                            inner: InnerEvmContext { db, .. },
                            ..
                        },
                    ..
                } = evm.into_context();

                let evm_context = EvmContext {
                    debug: None,
                    state: db.0.state,
                };

                match result {
                    Ok(result) => (evm_context, result),
                    Err(error) => {
                        return ExecutionResultWithContext {
                            result: Err(TransactionError::from(error).into()),
                            evm_context,
                        };
                    }
                }
            }
        };

        let state = &mut evm_context.state;

        self.state_diff.apply_diff(state_diff.clone());

        state.commit(state_diff);

        self.header.gas_used += result.gas_used();

        if let Some(BlobGas { gas_used, .. }) = self.header.blob_gas.as_mut() {
            *gas_used += blob_gas_used;
        }

        let receipt = receipt_builder.build_receipt(&self.header, &transaction, &result, spec_id);
        let receipt = TransactionReceipt::new(
            receipt,
            &transaction,
            &result,
            self.transactions.len() as u64,
            self.header.base_fee.unwrap_or(U256::ZERO),
            spec_id,
        );
        self.receipts.push(receipt);

        self.transactions.push(transaction);

        ExecutionResultWithContext {
            result: Ok(result),
            evm_context,
        }
    }
}
