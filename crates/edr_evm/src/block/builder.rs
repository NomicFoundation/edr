use std::{
    fmt::Debug,
    time::{SystemTime, UNIX_EPOCH},
};

use edr_eth::{
    block::{BlobGas, BlockOptions, PartialHeader},
    eips::eip4844,
    l1,
    log::ExecutionLog,
    receipt::{Receipt as _, TransactionReceipt},
    result::{ExecutionResult, InvalidTransaction, ResultAndState},
    spec::ChainSpec,
    transaction::{ExecutableTransaction as _, Transaction as _, TransactionValidation},
    trie::{ordered_trie_root, KECCAK_NULL_RLP},
    withdrawal::Withdrawal,
    Address, Bloom, U256,
};
use revm::Evm;

use super::local::LocalBlock;
use crate::{
    blockchain::SyncBlockchain,
    config::{CfgEnv, Env},
    debug::{DebugContext, EvmContext},
    receipt::ExecutionReceiptBuilder as _,
    spec::{BlockEnvConstructor, RuntimeSpec},
    state::{
        AccountModifierFn, DatabaseComponents, State, StateCommit, StateDebug, StateDiff,
        SyncState, WrapDatabaseRef,
    },
    transaction::TransactionError,
    SyncBlock,
};

/// An error caused during construction of a block builder.
#[derive(Debug, thiserror::Error)]
pub enum BlockBuilderCreationError<ChainSpecT>
where
    ChainSpecT: RuntimeSpec<Hardfork: Debug>,
{
    /// Unsupported hardfork. Hardforks older than Byzantium are not supported
    #[error("Unsupported hardfork: {0:?}. Hardforks older than Byzantium are not supported.")]
    UnsupportedHardfork(ChainSpecT::Hardfork),
}

/// An error caused during execution of a transaction while building a block.
#[derive(Debug, thiserror::Error)]
pub enum BlockTransactionError<ChainSpecT, BlockchainErrorT, StateErrorT>
where
    ChainSpecT: ChainSpec,
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
    ChainSpecT: RuntimeSpec<
        SignedTransaction: TransactionValidation<ValidationError: From<InvalidTransaction>>,
    >,
    BlockchainErrorT,
    StateErrorT,
    DebugDataT,
    StateT: State,
> {
    /// The result of executing the transaction.
    pub result: Result<
        ExecutionResult<ChainSpecT::HaltReason>,
        BlockTransactionError<ChainSpecT, BlockchainErrorT, StateErrorT>,
    >,
    /// The context in which the transaction was executed.
    pub evm_context: EvmContext<'evm, ChainSpecT, BlockchainErrorT, DebugDataT, StateT>,
}

/// The result of building a block, using the [`BlockBuilder`].
pub struct BuildBlockResult<ChainSpecT: RuntimeSpec> {
    /// Built block
    pub block: LocalBlock<ChainSpecT>,
    /// State diff
    pub state_diff: StateDiff,
}

/// A builder for constructing Ethereum blocks.
pub struct BlockBuilder<ChainSpecT: RuntimeSpec> {
    cfg: CfgEnv,
    hardfork: ChainSpecT::Hardfork,
    header: PartialHeader,
    transactions: Vec<ChainSpecT::SignedTransaction>,
    state_diff: StateDiff,
    receipts: Vec<TransactionReceipt<ChainSpecT::ExecutionReceipt<ExecutionLog>, ExecutionLog>>,
    parent_gas_limit: Option<u64>,
    withdrawals: Option<Vec<Withdrawal>>,
}

impl<ChainSpecT> BlockBuilder<ChainSpecT>
where
    ChainSpecT: RuntimeSpec<Hardfork: Debug>,
{
    /// Creates an intance of [`BlockBuilder`].
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn new<BlockchainErrorT>(
        cfg: CfgEnv,
        hardfork: ChainSpecT::Hardfork,
        parent: &dyn SyncBlock<ChainSpecT, Error = BlockchainErrorT>,
        mut options: BlockOptions,
    ) -> Result<Self, BlockBuilderCreationError<ChainSpecT>> {
        let evm_spec_id = hardfork.into();
        if evm_spec_id < l1::SpecId::BYZANTIUM {
            return Err(BlockBuilderCreationError::UnsupportedHardfork(hardfork));
        }

        let parent_header = parent.header();
        let parent_gas_limit = if options.gas_limit.is_none() {
            Some(parent_header.gas_limit)
        } else {
            None
        };

        let withdrawals = std::mem::take(&mut options.withdrawals).or_else(|| {
            if evm_spec_id >= l1::SpecId::SHANGHAI {
                Some(Vec::new())
            } else {
                None
            }
        });

        options.parent_hash = Some(*parent.hash());
        let header = PartialHeader::new::<ChainSpecT>(hardfork, options, Some(parent_header));

        Ok(Self {
            cfg,
            hardfork,
            header,
            transactions: Vec::new(),
            state_diff: StateDiff::default(),
            receipts: Vec::new(),
            parent_gas_limit,
            withdrawals,
        })
    }
}

impl<ChainSpecT: RuntimeSpec> BlockBuilder<ChainSpecT> {
    /// Retrieves the config of the block builder.
    pub fn config(&self) -> &CfgEnv {
        &self.cfg
    }

    /// Retrieves the hardfork of the block builder.
    pub fn hardfork(&self) -> ChainSpecT::Hardfork {
        self.hardfork
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
}

impl<ChainSpecT: RuntimeSpec> BlockBuilder<ChainSpecT> {
    /// Finalizes the block, returning the block and the callers of the
    /// transactions.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn finalize<StateT, StateErrorT: Debug + Send>(
        mut self,
        state: &mut StateT,
        rewards: Vec<(Address, U256)>,
    ) -> Result<BuildBlockResult<ChainSpecT>, StateErrorT>
    where
        StateT: SyncState<StateErrorT> + ?Sized,
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
    ChainSpecT: RuntimeSpec<
        Block: Default,
        SignedTransaction: Clone
                               + Default
                               + TransactionValidation<ValidationError: From<InvalidTransaction>>,
    >,
{
    /// Adds a pending transaction to
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub fn add_transaction<'blockchain, 'evm, DebugDataT, StateT, BlockchainErrorT, StateErrorT>(
        &mut self,
        blockchain: &'blockchain dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateErrorT>,
        state: StateT,
        transaction: ChainSpecT::SignedTransaction,
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
        StateT: State<Error = StateErrorT> + StateCommit + StateDebug<Error = StateErrorT>,
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
            if block_blob_gas_used + blob_gas_used > eip4844::MAX_BLOB_GAS_PER_BLOCK {
                return ExecutionResultWithContext {
                    result: Err(BlockTransactionError::ExceedsBlockBlobGasLimit),
                    evm_context: EvmContext {
                        debug: debug_context,
                        state,
                    },
                };
            }
        }

        let block = ChainSpecT::Block::new_block_env(&self.header, self.hardfork.into());

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

        let env = Env::boxed(self.cfg.clone(), block, transaction.clone());
        let db = WrapDatabaseRef(DatabaseComponents { blockchain, state });

        let (
            mut evm_context,
            ResultAndState {
                result,
                state: state_diff,
            },
        ) = {
            if let Some(debug_context) = debug_context {
                let mut evm = Evm::<ChainSpecT::EvmWiring<_, _>>::builder()
                    .with_db(db)
                    .with_external_context(debug_context.data)
                    .with_env(env)
                    .with_spec_id(self.hardfork)
                    .append_handler_register(debug_context.register_handles_fn)
                    .build();

                let result = evm.transact();
                let revm::Context {
                    evm:
                        revm::EvmContext {
                            inner: revm::InnerEvmContext { db, .. },
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
                let mut evm = Evm::<ChainSpecT::EvmWiring<_, ()>>::builder()
                    .with_db(db)
                    .with_external_context(())
                    .with_env(env)
                    .with_spec_id(self.hardfork)
                    .build();

                let result = evm.transact();
                let revm::Context {
                    evm:
                        revm::EvmContext {
                            inner: revm::InnerEvmContext { db, .. },
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

        let receipt =
            receipt_builder.build_receipt(&self.header, &transaction, &result, self.hardfork);
        let receipt = TransactionReceipt::new(
            receipt,
            &transaction,
            &result,
            self.transactions.len() as u64,
            self.header.base_fee.unwrap_or(U256::ZERO),
            self.hardfork,
        );
        self.receipts.push(receipt);

        self.transactions.push(transaction);

        ExecutionResultWithContext {
            result: Ok(result),
            evm_context,
        }
    }
}
