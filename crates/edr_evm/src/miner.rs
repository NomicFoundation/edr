use std::{cmp::Ordering, fmt::Debug};

use edr_block_api::{Block as _, GenesisBlockFactory};
use edr_block_builder_api::{
    BlockBuilder, BlockBuilderCreationError, BlockInputs, BlockTransactionError, DynBlockchain,
    BuiltBlockAndState, PrecompileFn, WrapDatabaseRef,
};
use edr_block_header::{calculate_next_base_fee_per_blob_gas, HeaderOverrides, PartialHeader};
use edr_chain_spec::{
    ChainSpec, EvmTransactionValidationError, ExecutableTransaction, HardforkChainSpec,
    TransactionValidation,
};
use edr_chain_spec_block::BlockChainSpec;
use edr_database_components::DatabaseComponents;
use edr_evm_spec::{
    config::EvmConfig, ContextForChainSpec, DatabaseComponentError, Inspector, TransactionError,
};
use edr_primitives::{Address, HashMap};
use edr_signer::SignatureError;
use edr_state_api::SyncState;
use serde::{Deserialize, Serialize};

use crate::{mempool::OrderedTransaction, MemPool};

/// Helper type for a chain-specific [`MineBlockResultAndState`].
pub type MineBlockResultAndStateForChainSpec<ChainSpecT, StateErrorT> = BuiltBlockAndState<
    <ChainSpecT as ChainSpec>::HaltReason,
    <ChainSpecT as GenesisBlockFactory>::LocalBlock,
    StateErrorT,
>;

/// The type of ordering to use when selecting blocks to mine.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum MineOrdering {
    /// Insertion order
    Fifo,
    /// Effective miner fee
    Priority,
}

/// Helper type for a chain-specific [`MineBlockError`].
pub type MineBlockErrorForChainSpec<BlockchainErrorT, ChainSpecT, StateErrorT> = MineBlockError<
    BlockchainErrorT,
    <ChainSpecT as HardforkChainSpec>::Hardfork,
    StateErrorT,
    <<ChainSpecT as ChainSpec>::SignedTransaction as TransactionValidation>::ValidationError,
>;

/// An error that occurred while mining a block.
#[derive(Debug, thiserror::Error)]
pub enum MineBlockError<BlockchainErrorT, HardforkT, StateErrorT, TransactionValidationErrorT> {
    /// An error that occurred while constructing a block builder.
    #[error(transparent)]
    BlockBuilderCreation(
        #[from]
        BlockBuilderCreationError<
            DatabaseComponentError<BlockchainErrorT, StateErrorT>,
            HardforkT,
        >,
    ),
    /// An error that occurred while executing a transaction.
    #[error(transparent)]
    BlockTransaction(
        #[from]
        BlockTransactionError<
            DatabaseComponentError<BlockchainErrorT, StateErrorT>,
            TransactionValidationErrorT,
        >,
    ),
    /// An error that occurred while finalizing a block.
    #[error(transparent)]
    BlockFinalize(StateErrorT),
    /// A blockchain error
    #[error(transparent)]
    Blockchain(BlockchainErrorT),
    /// The block is expected to have a prevrandao, as the executor's config is
    /// on a post-merge hardfork.
    #[error("Post-merge transaction is missing prevrandao")]
    MissingPrevrandao,
}

/// Mines a block using as many transactions as can fit in it.
#[allow(clippy::too_many_arguments)]
// `DebugContext` cannot be simplified further
#[allow(clippy::type_complexity)]
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub fn mine_block<
    BlockchainErrorT,
    ChainSpecT: BlockChainSpec<
        SignedTransaction: 'static
                               + Clone
                               + Debug
                               + TransactionValidation<
            ValidationError: From<EvmTransactionValidationError> + PartialEq,
        >,
    >,
    InspectorT,
    StateErrorT,
>(
    blockchain: &dyn DynBlockchain<
        ChainSpecT::Receipt,
        ChainSpecT::Block,
        BlockchainErrorT,
        ChainSpecT::Hardfork,
        ChainSpecT::LocalBlock,
        ChainSpecT::SignedTransaction,
        StateErrorT,
    >,
    state: Box<dyn SyncState<StateErrorT>>,
    mem_pool: &MemPool<ChainSpecT::SignedTransaction>,
    evm_config: &EvmConfig,
    overrides: HeaderOverrides<ChainSpecT::Hardfork>,
    min_gas_price: u128,
    mine_ordering: MineOrdering,
    reward: u128,
    mut inspector: Option<&mut InspectorT>,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
) -> Result<
    BuiltBlockAndState<ChainSpecT::HaltReason, ChainSpecT::LocalBlock, StateErrorT>,
    MineBlockErrorForChainSpec<BlockchainErrorT, ChainSpecT, StateErrorT>,
>
where
    BlockchainErrorT: std::error::Error + Send,
    InspectorT: for<'inspector> Inspector<
        ContextForChainSpec<
            ChainSpecT,
            ChainSpecT::BlockEnv<'inspector, PartialHeader>,
            WrapDatabaseRef<
                DatabaseComponents<
                    &'inspector dyn DynBlockchain<
                        ChainSpecT::Receipt,
                        ChainSpecT::Block,
                        BlockchainErrorT,
                        ChainSpecT::Hardfork,
                        ChainSpecT::LocalBlock,
                        ChainSpecT::SignedTransaction,
                        StateErrorT,
                    >,
                    &'inspector dyn SyncState<StateErrorT>,
                >,
            >,
        >,
    >,
    StateErrorT: std::error::Error + Send,
{
    let block_inputs = BlockInputs::new(blockchain.hardfork());
    let mut block_builder = ChainSpecT::BlockBuilder::new_block_builder(
        blockchain,
        state,
        evm_config,
        block_inputs,
        overrides,
        custom_precompiles,
    )?;

    let mut pending_transactions = {
        type MineOrderComparator<SignedTransactionT> = dyn Fn(
                &OrderedTransaction<SignedTransactionT>,
                &OrderedTransaction<SignedTransactionT>,
            ) -> Ordering
            + Send;

        let base_fee = block_builder.header().base_fee;
        let comparator: Box<MineOrderComparator<ChainSpecT::SignedTransaction>> =
            match mine_ordering {
                MineOrdering::Fifo => Box::new(first_in_first_out_comparator),
                MineOrdering::Priority => {
                    Box::new(move |lhs, rhs| priority_comparator(lhs, rhs, base_fee))
                }
            };

        mem_pool.iter(comparator)
    };

    while let Some(transaction) = pending_transactions.next() {
        if *transaction.gas_price() < min_gas_price {
            pending_transactions.remove_caller(transaction.caller());
            continue;
        }

        let caller = *transaction.caller();

        {
            let result = if let Some(inspector) = inspector.as_mut() {
                block_builder.add_transaction_with_inspector(transaction, inspector)
            } else {
                block_builder.add_transaction(transaction)
            };

            if let Err(error) = result {
                match error {
                    BlockTransactionError::ExceedsBlockGasLimit => {
                        pending_transactions.remove_caller(&caller);
                    }
                    BlockTransactionError::Transaction(TransactionError::InvalidTransaction(
                        error,
                    )) if error
                        == EvmTransactionValidationError::GasPriceLessThanBasefee.into() =>
                    {
                        pending_transactions.remove_caller(&caller);
                    }
                    remainder => return Err(MineBlockError::BlockTransaction(remainder)),
                }
            }
        }
    }

    let beneficiary = block_builder.header().beneficiary;
    let rewards = vec![(beneficiary, reward)];

    block_builder
        .finalize_block(rewards)
        .map_err(MineBlockError::BlockFinalize)
}

/// Helper type for a chain-specific [`MineTransactionError`].
pub type MineTransactionErrorForChainSpec<ChainSpecT, BlockchainErrorT, StateErrorT> =
    MineTransactionError<
        BlockchainErrorT,
        <ChainSpecT as HardforkChainSpec>::Hardfork,
        StateErrorT,
        <<ChainSpecT as ChainSpec>::SignedTransaction as TransactionValidation>::ValidationError,
    >;

/// An error that occurred while mining a block with a single transaction.
#[derive(Debug, thiserror::Error)]
pub enum MineTransactionError<BlockchainErrorT, HardforkT, StateErrorT, TransactionValidationErrorT>
{
    /// An error that occurred while constructing a block builder.
    #[error(transparent)]
    BlockBuilderCreation(
        #[from]
        BlockBuilderCreationError<
            DatabaseComponentError<BlockchainErrorT, StateErrorT>,
            HardforkT,
        >,
    ),
    /// An error that occurred while executing a transaction.
    #[error(transparent)]
    BlockTransaction(
        #[from]
        BlockTransactionError<
            DatabaseComponentError<BlockchainErrorT, StateErrorT>,
            TransactionValidationErrorT,
        >,
    ),
    /// A blockchain error
    #[error(transparent)]
    Blockchain(BlockchainErrorT),
    /// The transaction's gas price is lower than the block's minimum gas price.
    #[error(
        "Transaction gasPrice ({actual}) is too low for the next block, which has a baseFeePerGas of {expected}"
    )]
    GasPriceTooLow {
        /// The minimum gas price.
        expected: u128,
        /// The actual gas price.
        actual: u128,
    },
    /// The transaction's max fee per gas is lower than the next block's base
    /// fee.
    #[error(
        "Transaction maxFeePerGas ({actual}) is too low for the next block, which has a baseFeePerGas of {expected}"
    )]
    MaxFeePerGasTooLow {
        /// The minimum max fee per gas.
        expected: u128,
        /// The actual max fee per gas.
        actual: u128,
    },
    /// The transaction's max fee per blob gas is lower than the next block's
    /// base fee.
    #[error(
        "Transaction maxFeePerBlobGas ({actual}) is too low for the next block, which has a baseFeePerBlobGas of {expected}"
    )]
    MaxFeePerBlobGasTooLow {
        /// The minimum max fee per blob gas.
        expected: u128,
        /// The actual max fee per blob gas.
        actual: u128,
    },
    /// The block is expected to have a prevrandao, as the executor's config is
    /// on a post-merge hardfork.
    #[error("Post-merge transaction is missing prevrandao")]
    MissingPrevrandao,
    /// The transaction nonce is too high.
    #[error(
        "Nonce too high. Expected nonce to be {expected} but got {actual}. Note that transactions can't be queued when automining."
    )]
    NonceTooHigh {
        /// The expected nonce.
        expected: u64,
        /// The actual nonce.
        actual: u64,
    },
    /// The transaction nonce is too high.
    #[error(
        "Nonce too low. Expected nonce to be {expected} but got {actual}. Note that transactions can't be queued when automining."
    )]
    NonceTooLow {
        /// The expected nonce.
        expected: u64,
        /// The actual nonce.
        actual: u64,
    },
    /// The transaction's priority fee is lower than the minimum gas price.
    #[error("Transaction gas price is {actual}, which is below the minimum of {expected}")]
    PriorityFeeTooLow {
        /// The minimum gas price.
        expected: u128,
        /// The actual max priority fee per gas.
        actual: u128,
    },
    /// Signature error
    #[error(transparent)]
    Signature(#[from] SignatureError),
    /// An error that occurred while querying state.
    #[error(transparent)]
    State(StateErrorT),
}

/// Mines a block with a single transaction.
///
/// If the transaction is invalid, returns an error.
#[allow(clippy::too_many_arguments)]
// `DebugContext` cannot be simplified further
#[allow(clippy::type_complexity)]
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub fn mine_block_with_single_transaction<
    'builder,
    BlockchainErrorT: std::error::Error + Send,
    ChainSpecT: BlockChainSpec,
    InspectorT,
    StateErrorT,
>(
    blockchain: &dyn DynBlockchain<
        ChainSpecT::Receipt,
        ChainSpecT::Block,
        BlockchainErrorT,
        ChainSpecT::Hardfork,
        ChainSpecT::LocalBlock,
        ChainSpecT::SignedTransaction,
        StateErrorT,
    >,
    state: Box<dyn SyncState<StateErrorT>>,
    transaction: ChainSpecT::SignedTransaction,
    evm_config: &EvmConfig,
    overrides: HeaderOverrides<ChainSpecT::Hardfork>,
    min_gas_price: u128,
    reward: u128,
    inspector: Option<&mut InspectorT>,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
) -> Result<
    BuiltBlockAndState<ChainSpecT::HaltReason, ChainSpecT::LocalBlock, StateErrorT>,
    MineTransactionErrorForChainSpec<ChainSpecT, BlockchainErrorT, StateErrorT>,
>
where
    InspectorT: for<'inspector> Inspector<
        ContextForChainSpec<
            ChainSpecT,
            ChainSpecT::BlockEnv<'inspector, PartialHeader>,
            WrapDatabaseRef<
                DatabaseComponents<
                    &'inspector dyn DynBlockchain<
                        ChainSpecT::Receipt,
                        ChainSpecT::Block,
                        BlockchainErrorT,
                        ChainSpecT::Hardfork,
                        ChainSpecT::LocalBlock,
                        ChainSpecT::SignedTransaction,
                        StateErrorT,
                    >,
                    &'inspector dyn SyncState<StateErrorT>,
                >,
            >,
        >,
    >,
    StateErrorT: std::error::Error,
{
    let max_priority_fee_per_gas = transaction
        .max_priority_fee_per_gas()
        .unwrap_or_else(|| transaction.gas_price());

    if *max_priority_fee_per_gas < min_gas_price {
        return Err(MineTransactionError::PriorityFeeTooLow {
            expected: min_gas_price,
            actual: *max_priority_fee_per_gas,
        });
    }

    if let Some(base_fee_per_gas) = overrides.base_fee {
        if let Some(max_fee_per_gas) = transaction.max_fee_per_gas() {
            if *max_fee_per_gas < base_fee_per_gas {
                return Err(MineTransactionError::MaxFeePerGasTooLow {
                    expected: base_fee_per_gas,
                    actual: *max_fee_per_gas,
                });
            }
        } else {
            let gas_price = transaction.gas_price();
            if *gas_price < base_fee_per_gas {
                return Err(MineTransactionError::GasPriceTooLow {
                    expected: base_fee_per_gas,
                    actual: *gas_price,
                });
            }
        }
    }

    let parent_block = blockchain
        .last_block()
        .map_err(MineTransactionError::Blockchain)?;

    let hardfork = blockchain.hardfork();

    if let Some(max_fee_per_blob_gas) = transaction.max_fee_per_blob_gas() {
        let base_fee_per_blob_gas =
            calculate_next_base_fee_per_blob_gas(parent_block.block_header(), hardfork);
        if *max_fee_per_blob_gas < base_fee_per_blob_gas {
            return Err(MineTransactionError::MaxFeePerBlobGasTooLow {
                expected: base_fee_per_blob_gas,
                actual: *max_fee_per_blob_gas,
            });
        }
    }

    let sender = state
        .basic(*transaction.caller())
        .map_err(MineTransactionError::State)?
        .unwrap_or_default();

    // TODO: This is also checked by `revm`, so it can be simplified
    match transaction.nonce().cmp(&sender.nonce) {
        Ordering::Less => {
            return Err(MineTransactionError::NonceTooLow {
                expected: sender.nonce,
                actual: transaction.nonce(),
            });
        }
        Ordering::Equal => (),
        Ordering::Greater => {
            return Err(MineTransactionError::NonceTooHigh {
                expected: sender.nonce,
                actual: transaction.nonce(),
            });
        }
    }

    let mut block_builder = ChainSpecT::BlockBuilder::new_block_builder(
        blockchain,
        state,
        evm_config,
        BlockInputs::new(hardfork),
        overrides,
        custom_precompiles,
    )?;

    let beneficiary = block_builder.header().beneficiary;
    let rewards = vec![(beneficiary, reward)];

    if let Some(inspector) = inspector {
        block_builder.add_transaction_with_inspector(transaction, inspector)?;
    } else {
        block_builder.add_transaction(transaction)?;
    }

    block_builder
        .finalize_block(rewards)
        .map_err(MineTransactionError::State)
}

fn effective_miner_fee(transaction: &impl ExecutableTransaction, base_fee: Option<u128>) -> u128 {
    let max_fee_per_gas = transaction.gas_price();
    let max_priority_fee_per_gas = *transaction
        .max_priority_fee_per_gas()
        .unwrap_or(max_fee_per_gas);

    base_fee.map_or(*max_fee_per_gas, |base_fee| {
        max_priority_fee_per_gas.min(*max_fee_per_gas - base_fee)
    })
}

fn first_in_first_out_comparator<SignedTransactionT: ExecutableTransaction>(
    lhs: &OrderedTransaction<SignedTransactionT>,
    rhs: &OrderedTransaction<SignedTransactionT>,
) -> Ordering {
    lhs.order_id().cmp(&rhs.order_id())
}

fn priority_comparator<SignedTransactionT: ExecutableTransaction>(
    lhs: &OrderedTransaction<SignedTransactionT>,
    rhs: &OrderedTransaction<SignedTransactionT>,
    base_fee: Option<u128>,
) -> Ordering {
    let effective_miner_fee =
        move |transaction: &SignedTransactionT| effective_miner_fee(transaction, base_fee);

    // Invert lhs and rhs to get decreasing order by effective miner fee
    let ordering = effective_miner_fee(rhs.pending()).cmp(&effective_miner_fee(lhs.pending()));

    // If two txs have the same effective miner fee we want to sort them
    // in increasing order by orderId
    if ordering == Ordering::Equal {
        lhs.order_id().cmp(&rhs.order_id())
    } else {
        ordering
    }
}

#[cfg(test)]
mod tests {
    use edr_primitives::U256;
    use edr_state_api::account::AccountInfo;

    use super::*;
    use crate::test_utils::{
        dummy_eip1559_transaction, dummy_eip155_transaction_with_price, MemPoolTestFixture,
    };

    #[test]
    fn fifo_ordering() -> anyhow::Result<()> {
        let sender1 = Address::random();
        let sender2 = Address::random();
        let sender3 = Address::random();

        let account_with_balance = AccountInfo {
            balance: U256::from(100_000_000u64),
            ..AccountInfo::default()
        };
        let mut fixture = MemPoolTestFixture::with_accounts(&[
            (sender1, account_with_balance.clone()),
            (sender2, account_with_balance.clone()),
            (sender3, account_with_balance),
        ]);

        let base_fee = Some(15u128);

        let transaction1 = dummy_eip155_transaction_with_price(sender1, 0, 111)?;
        assert_eq!(effective_miner_fee(&transaction1, base_fee), 96);
        fixture.add_transaction(transaction1.clone())?;

        let transaction2 = dummy_eip1559_transaction(sender2, 0, 120, 100)?;
        assert_eq!(effective_miner_fee(&transaction2, base_fee), 100);
        fixture.add_transaction(transaction2.clone())?;

        let transaction3 = dummy_eip1559_transaction(sender3, 0, 140, 110)?;
        assert_eq!(effective_miner_fee(&transaction3, base_fee), 110);
        fixture.add_transaction(transaction3.clone())?;

        let mut ordered_transactions = fixture.mem_pool.iter(first_in_first_out_comparator);

        assert_eq!(ordered_transactions.next(), Some(transaction1));
        assert_eq!(ordered_transactions.next(), Some(transaction2));
        assert_eq!(ordered_transactions.next(), Some(transaction3));

        Ok(())
    }

    #[test]
    fn priority_ordering_gas_price_without_base_fee() -> anyhow::Result<()> {
        let sender1 = Address::random();
        let sender2 = Address::random();
        let sender3 = Address::random();
        let sender4 = Address::random();

        let account_with_balance = AccountInfo {
            balance: U256::from(100_000_000u64),
            ..AccountInfo::default()
        };
        let mut fixture = MemPoolTestFixture::with_accounts(&[
            (sender1, account_with_balance.clone()),
            (sender2, account_with_balance.clone()),
            (sender3, account_with_balance.clone()),
            (sender4, account_with_balance),
        ]);

        let transaction1 = dummy_eip155_transaction_with_price(sender1, 0, 123)?;
        fixture.add_transaction(transaction1.clone())?;

        let transaction2 = dummy_eip155_transaction_with_price(sender2, 0, 1_000)?;
        fixture.add_transaction(transaction2.clone())?;

        // This has the same gasPrice than tx2, but arrived later, so it's placed later
        // in the queue
        let transaction3 = dummy_eip155_transaction_with_price(sender3, 0, 1_000)?;
        fixture.add_transaction(transaction3.clone())?;

        let transaction4 = dummy_eip155_transaction_with_price(sender4, 0, 2_000)?;
        fixture.add_transaction(transaction4.clone())?;

        let mut ordered_transactions = fixture
            .mem_pool
            .iter(|lhs, rhs| priority_comparator(lhs, rhs, None));

        assert_eq!(ordered_transactions.next(), Some(transaction4));
        assert_eq!(ordered_transactions.next(), Some(transaction2));
        assert_eq!(ordered_transactions.next(), Some(transaction3));
        assert_eq!(ordered_transactions.next(), Some(transaction1));

        Ok(())
    }

    #[test]
    fn priority_ordering_gas_price_with_base_fee() -> anyhow::Result<()> {
        let sender1 = Address::random();
        let sender2 = Address::random();
        let sender3 = Address::random();
        let sender4 = Address::random();
        let sender5 = Address::random();

        let account_with_balance = AccountInfo {
            balance: U256::from(100_000_000u64),
            ..AccountInfo::default()
        };
        let mut fixture = MemPoolTestFixture::with_accounts(&[
            (sender1, account_with_balance.clone()),
            (sender2, account_with_balance.clone()),
            (sender3, account_with_balance.clone()),
            (sender4, account_with_balance.clone()),
            (sender5, account_with_balance),
        ]);

        let base_fee = Some(15u128);

        let transaction1 = dummy_eip155_transaction_with_price(sender1, 0, 111)?;
        assert_eq!(effective_miner_fee(&transaction1, base_fee), 96);
        fixture.add_transaction(transaction1.clone())?;

        let transaction2 = dummy_eip1559_transaction(sender2, 0, 120, 100)?;
        assert_eq!(effective_miner_fee(&transaction2, base_fee), 100);
        fixture.add_transaction(transaction2.clone())?;

        let transaction3 = dummy_eip1559_transaction(sender3, 0, 140, 110)?;
        assert_eq!(effective_miner_fee(&transaction3, base_fee), 110);
        fixture.add_transaction(transaction3.clone())?;

        let transaction4 = dummy_eip1559_transaction(sender4, 0, 140, 130)?;
        assert_eq!(effective_miner_fee(&transaction4, base_fee), 125);
        fixture.add_transaction(transaction4.clone())?;

        let transaction5 = dummy_eip155_transaction_with_price(sender5, 0, 170)?;
        assert_eq!(effective_miner_fee(&transaction5, base_fee), 155);
        fixture.add_transaction(transaction5.clone())?;

        let mut ordered_transactions = fixture
            .mem_pool
            .iter(|lhs, rhs| priority_comparator(lhs, rhs, base_fee));

        assert_eq!(ordered_transactions.next(), Some(transaction5));
        assert_eq!(ordered_transactions.next(), Some(transaction4));
        assert_eq!(ordered_transactions.next(), Some(transaction3));
        assert_eq!(ordered_transactions.next(), Some(transaction2));
        assert_eq!(ordered_transactions.next(), Some(transaction1));

        Ok(())
    }

    #[test]
    fn ordering_remove_caller() -> anyhow::Result<()> {
        let sender1 = Address::random();
        let sender2 = Address::random();
        let sender3 = Address::random();
        let sender4 = Address::random();

        let account_with_balance = AccountInfo {
            balance: U256::from(100_000_000u64),
            ..AccountInfo::default()
        };
        let mut fixture = MemPoolTestFixture::with_accounts(&[
            (sender1, account_with_balance.clone()),
            (sender2, account_with_balance.clone()),
            (sender3, account_with_balance.clone()),
            (sender4, account_with_balance),
        ]);

        // Insert 9 transactions sequentially (no for loop)
        let transaction1 = dummy_eip155_transaction_with_price(sender1, 0, 100)?;
        fixture.add_transaction(transaction1.clone())?;

        let transaction2 = dummy_eip155_transaction_with_price(sender1, 1, 99)?;
        fixture.add_transaction(transaction2.clone())?;

        let transaction3 = dummy_eip155_transaction_with_price(sender2, 0, 98)?;
        fixture.add_transaction(transaction3.clone())?;

        let transaction4 = dummy_eip155_transaction_with_price(sender2, 1, 97)?;
        fixture.add_transaction(transaction4.clone())?;

        let transaction5 = dummy_eip155_transaction_with_price(sender3, 0, 96)?;
        fixture.add_transaction(transaction5.clone())?;

        let transaction6 = dummy_eip155_transaction_with_price(sender3, 1, 95)?;
        fixture.add_transaction(transaction6.clone())?;

        let transaction7 = dummy_eip155_transaction_with_price(sender3, 2, 94)?;
        fixture.add_transaction(transaction7.clone())?;

        let transaction8 = dummy_eip155_transaction_with_price(sender3, 3, 93)?;
        fixture.add_transaction(transaction8.clone())?;

        let transaction9 = dummy_eip155_transaction_with_price(sender4, 0, 92)?;
        fixture.add_transaction(transaction9.clone())?;

        let transaction10 = dummy_eip155_transaction_with_price(sender4, 1, 91)?;
        fixture.add_transaction(transaction10.clone())?;

        let mut ordered_transactions = fixture
            .mem_pool
            .iter(|lhs, rhs| priority_comparator(lhs, rhs, None));

        assert_eq!(ordered_transactions.next(), Some(transaction1));
        assert_eq!(ordered_transactions.next(), Some(transaction2));
        assert_eq!(ordered_transactions.next(), Some(transaction3));

        // Remove all transactions for sender 2
        ordered_transactions.remove_caller(&sender2);

        assert_eq!(ordered_transactions.next(), Some(transaction5));
        assert_eq!(ordered_transactions.next(), Some(transaction6));
        assert_eq!(ordered_transactions.next(), Some(transaction7));

        // Remove all transactions for sender 3
        ordered_transactions.remove_caller(&sender3);

        assert_eq!(ordered_transactions.next(), Some(transaction9));
        assert_eq!(ordered_transactions.next(), Some(transaction10));

        Ok(())
    }
}
