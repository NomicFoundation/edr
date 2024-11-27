use std::{
    fmt::Debug,
    time::{SystemTime, UNIX_EPOCH},
};

use edr_eth::{
    block::{BlobGas, BlockOptions, PartialHeader},
    eips::eip4844,
    l1::{self, L1ChainSpec},
    log::ExecutionLog,
    receipt::{Receipt as _, TransactionReceipt},
    result::{ExecutionResult, ResultAndState},
    transaction::{ExecutableTransaction as _, Transaction as _},
    trie::{ordered_trie_root, KECCAK_NULL_RLP},
    withdrawal::Withdrawal,
    Address, Bloom, U256,
};
use revm::Evm;

use super::{BlockBuilder, BlockBuilderAndError, BlockTransactionError};
use crate::{
    blockchain::SyncBlockchain,
    config::{CfgEnv, Env},
    debug::DebugContext,
    receipt::{self, ExecutionReceiptBuilder as _},
    spec::{BlockEnvConstructor as _, RuntimeSpec, SyncRuntimeSpec},
    state::{AccountModifierFn, DatabaseComponents, StateDiff, SyncState, WrapDatabaseRef},
    transaction::TransactionError,
    BlockBuilderCreationError, LocalBlock, MineBlockResultAndState,
};

/// A builder for constructing Ethereum L1 blocks.
pub struct EthBlockBuilder<'blockchain, BlockchainErrorT, ChainSpecT, DebugDataT, StateErrorT>
where
    ChainSpecT: RuntimeSpec,
{
    blockchain: &'blockchain dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateErrorT>,
    cfg: CfgEnv,
    debug_context: Option<
        DebugContext<
            'blockchain,
            ChainSpecT,
            BlockchainErrorT,
            DebugDataT,
            Box<dyn SyncState<StateErrorT>>,
        >,
    >,
    hardfork: ChainSpecT::Hardfork,
    header: PartialHeader,
    parent_gas_limit: Option<u64>,
    receipts: Vec<TransactionReceipt<ChainSpecT::ExecutionReceipt<ExecutionLog>, ExecutionLog>>,
    state: Box<dyn SyncState<StateErrorT>>,
    state_diff: StateDiff,
    transactions: Vec<ChainSpecT::SignedTransaction>,
    transaction_results: Vec<ExecutionResult<ChainSpecT::HaltReason>>,
    withdrawals: Option<Vec<Withdrawal>>,
}

impl<'blockchain, BlockchainErrorT, ChainSpecT, DebugDataT, StateErrorT>
    EthBlockBuilder<'blockchain, BlockchainErrorT, ChainSpecT, DebugDataT, StateErrorT>
where
    ChainSpecT: RuntimeSpec,
{
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
}

impl<'blockchain, BlockchainErrorT, ChainSpecT, DebugDataT, StateErrorT>
    BlockBuilder<'blockchain, ChainSpecT, DebugDataT>
    for EthBlockBuilder<'blockchain, BlockchainErrorT, ChainSpecT, DebugDataT, StateErrorT>
where
    ChainSpecT: SyncRuntimeSpec<Hardfork: Debug>,
    StateErrorT: Debug + Send,
{
    type BlockchainError = BlockchainErrorT;

    type StateError = StateErrorT;

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn new_block_builder(
        blockchain: &'blockchain dyn SyncBlockchain<
            ChainSpecT,
            Self::BlockchainError,
            Self::StateError,
        >,
        state: Box<dyn SyncState<Self::StateError>>,
        hardfork: ChainSpecT::Hardfork,
        cfg: CfgEnv,
        mut options: BlockOptions,
        debug_context: Option<
            DebugContext<
                'blockchain,
                ChainSpecT,
                Self::BlockchainError,
                DebugDataT,
                Box<dyn SyncState<Self::StateError>>,
            >,
        >,
    ) -> Result<Self, BlockBuilderCreationError<Self::BlockchainError, ChainSpecT::Hardfork>> {
        let parent_block = blockchain
            .last_block()
            .map_err(BlockBuilderCreationError::Blockchain)?;

        let eth_hardfork = hardfork.into();
        if eth_hardfork < l1::SpecId::BYZANTIUM {
            return Err(BlockBuilderCreationError::UnsupportedHardfork(hardfork));
        }

        let parent_header = parent_block.header();
        let parent_gas_limit = if options.gas_limit.is_none() {
            Some(parent_header.gas_limit)
        } else {
            None
        };

        let withdrawals = std::mem::take(&mut options.withdrawals).or_else(|| {
            if eth_hardfork >= l1::SpecId::SHANGHAI {
                Some(Vec::new())
            } else {
                None
            }
        });

        options.parent_hash = Some(*parent_block.hash());
        let header = PartialHeader::new::<ChainSpecT>(hardfork, options, Some(parent_header));

        Ok(Self {
            blockchain,
            cfg,
            debug_context,
            hardfork,
            header,
            parent_gas_limit,
            receipts: Vec::new(),
            state,
            state_diff: StateDiff::default(),
            transactions: Vec::new(),
            transaction_results: Vec::new(),
            withdrawals,
        })
    }

    fn header(&self) -> &PartialHeader {
        &self.header
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn add_transaction(
        mut self,
        transaction: ChainSpecT::SignedTransaction,
    ) -> Result<
        Self,
        BlockBuilderAndError<
            Self,
            BlockTransactionError<ChainSpecT, Self::BlockchainError, Self::StateError>,
        >,
    > {
        // The transaction's gas limit cannot be greater than the remaining gas in the
        // block
        if transaction.gas_limit() > self.gas_remaining() {
            return Err(BlockBuilderAndError {
                block_builder: self,
                error: BlockTransactionError::ExceedsBlockGasLimit,
            });
        }

        let blob_gas_used = transaction.total_blob_gas().unwrap_or_default();
        if let Some(BlobGas {
            gas_used: block_blob_gas_used,
            ..
        }) = self.header.blob_gas.as_ref()
        {
            if block_blob_gas_used + blob_gas_used > eip4844::MAX_BLOB_GAS_PER_BLOCK {
                return Err(BlockBuilderAndError {
                    block_builder: self,
                    error: BlockTransactionError::ExceedsBlockBlobGasLimit,
                });
            }
        }

        let block = ChainSpecT::Block::new_block_env(&self.header, self.hardfork.into());

        let receipt_builder =
            match ChainSpecT::ReceiptBuilder::new_receipt_builder(&self.state, &transaction) {
                Ok(receipt_builder) => receipt_builder,
                Err(error) => {
                    return Err(BlockBuilderAndError {
                        block_builder: self,
                        error: TransactionError::State(error).into(),
                    });
                }
            };

        let env = Env::boxed(self.cfg.clone(), block, transaction.clone());

        let Self {
            blockchain,
            debug_context,
            hardfork,
            state,
            ..
        } = self;
        let db = WrapDatabaseRef(DatabaseComponents { blockchain, state });

        let ResultAndState {
            result: transaction_result,
            state: state_diff,
        } = {
            if let Some(debug_context) = debug_context {
                let mut evm = Evm::<ChainSpecT::EvmWiring<_, _>>::builder()
                    .with_db(db)
                    .with_external_context(debug_context.data)
                    .with_env(env)
                    .with_spec_id(hardfork)
                    .append_handler_register(debug_context.register_handles_fn)
                    .build();

                let result = evm.transact();

                let revm::Context {
                    evm:
                        revm::EvmContext {
                            inner:
                                revm::InnerEvmContext {
                                    db,
                                    chain: chain_context,
                                    ..
                                },
                            ..
                        },
                    external,
                } = evm.into_context();

                // Reconstruct self for moved values
                self.debug_context = Some(DebugContext {
                    data: external,
                    register_handles_fn: debug_context.register_handles_fn,
                });
                self.state = db.0.state;

                match result {
                    Ok(result) => result,
                    Err(error) => {
                        return Err(BlockBuilderAndError {
                            block_builder: self,
                            error: TransactionError::from(error).into(),
                        });
                    }
                }
            } else {
                let mut evm = Evm::<ChainSpecT::EvmWiring<_, ()>>::builder()
                    .with_db(db)
                    .with_external_context(())
                    .with_env(env)
                    .with_spec_id(hardfork)
                    .build();

                let result = evm.transact();

                let revm::Context {
                    evm:
                        revm::EvmContext {
                            inner:
                                revm::InnerEvmContext {
                                    db,
                                    chain: chain_context,
                                    ..
                                },
                            ..
                        },
                    ..
                } = evm.into_context();

                // Reconstruct self for moved values
                self.debug_context = None;
                self.state = db.0.state;

                match result {
                    Ok(result) => result,
                    Err(error) => {
                        return Err(BlockBuilderAndError {
                            block_builder: self,
                            error: TransactionError::from(error).into(),
                        });
                    }
                }
            }
        };

        self.state_diff.apply_diff(state_diff.clone());

        self.state.commit(state_diff);

        self.header.gas_used += transaction_result.gas_used();

        if let Some(BlobGas { gas_used, .. }) = self.header.blob_gas.as_mut() {
            *gas_used += blob_gas_used;
        }

        let receipt = receipt_builder.build_receipt(
            &self.header,
            &transaction,
            &transaction_result,
            self.hardfork,
        );
        let receipt = TransactionReceipt::new(
            receipt,
            &transaction,
            &transaction_result,
            self.transactions.len() as u64,
            self.header.base_fee.unwrap_or(U256::ZERO),
            self.hardfork,
        );
        self.receipts.push(receipt);

        self.transactions.push(transaction);
        self.transaction_results.push(transaction_result);

        Ok(self)
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn finalize(
        mut self,
        rewards: Vec<(Address, U256)>,
    ) -> Result<MineBlockResultAndState<ChainSpecT, Self::StateError>, Self::StateError> {
        for (address, reward) in rewards {
            if reward > U256::ZERO {
                let account_info = self.state.modify_account(
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
        let block = LocalBlock::new(
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

// impl<ChainSpecT> Builder<ChainSpecT> where
//     ChainSpecT: RuntimeSpec<
//         Block: Default,
//         SignedTransaction: Clone
//                                + Default
//                                + TransactionValidation<ValidationError:
//                                  From<InvalidTransaction>>,
//     >
// {
// }
