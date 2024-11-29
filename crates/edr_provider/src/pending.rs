use std::{collections::BTreeMap, sync::Arc};

use derive_where::derive_where;
use edr_eth::{transaction::ExecutableTransaction as _, HashSet, B256, U256};
use edr_evm::{
    blockchain::{BlockHash, Blockchain, BlockchainError, BlockchainMut, SyncBlockchain},
    spec::SyncRuntimeSpec,
    state::{StateDiff, StateError, StateOverride, SyncState},
    BlockAndTotalDifficulty, BlockReceipt, EthLocalBlock, SyncBlock,
};

/// A blockchain with a pending block.
///
/// # Panics
///
/// Panics if a state override is provided to `state_at_block_number` for the
/// pending block; or if the `BlockchainMut` methods are called.
///
/// WORKAROUND: This struct needs to implement all sub-traits of
/// [`SyncBlockchain`] because we cannot upcast the trait at its usage site
/// <https://github.com/NomicFoundation/edr/issues/284>
#[derive_where(Debug)]
pub(crate) struct BlockchainWithPending<'blockchain, ChainSpecT: SyncRuntimeSpec> {
    blockchain:
        &'blockchain dyn SyncBlockchain<ChainSpecT, BlockchainError<ChainSpecT>, StateError>,
    pending_block: Arc<dyn SyncBlock<ChainSpecT, Error = BlockchainError<ChainSpecT>>>,
    pending_state_diff: StateDiff,
}

impl<'blockchain, ChainSpecT: SyncRuntimeSpec> BlockchainWithPending<'blockchain, ChainSpecT> {
    /// Constructs a new instance with the provided blockchain and pending
    /// block.
    pub fn new(
        blockchain: &'blockchain dyn SyncBlockchain<
            ChainSpecT,
            BlockchainError<ChainSpecT>,
            StateError,
        >,
        pending_block: EthLocalBlock<ChainSpecT>,
        pending_state_diff: StateDiff,
    ) -> Self {
        Self {
            blockchain,
            pending_block: Arc::new(pending_block),
            pending_state_diff,
        }
    }
}

impl<'blockchain, ChainSpecT: SyncRuntimeSpec> Blockchain<ChainSpecT>
    for BlockchainWithPending<'blockchain, ChainSpecT>
{
    type BlockchainError = BlockchainError<ChainSpecT>;

    type StateError = StateError;

    fn block_by_hash(
        &self,
        hash: &B256,
    ) -> Result<
        Option<Arc<dyn SyncBlock<ChainSpecT, Error = Self::BlockchainError>>>,
        Self::BlockchainError,
    > {
        if hash == self.pending_block.block_hash() {
            Ok(Some(self.pending_block.clone()))
        } else {
            self.blockchain.block_by_hash(hash)
        }
    }

    fn block_by_number(
        &self,
        number: u64,
    ) -> Result<
        Option<Arc<dyn SyncBlock<ChainSpecT, Error = Self::BlockchainError>>>,
        Self::BlockchainError,
    > {
        if number == self.pending_block.header().number {
            Ok(Some(self.pending_block.clone()))
        } else {
            self.blockchain.block_by_number(number)
        }
    }

    fn block_by_transaction_hash(
        &self,
        transaction_hash: &B256,
    ) -> Result<
        Option<Arc<dyn SyncBlock<ChainSpecT, Error = Self::BlockchainError>>>,
        Self::BlockchainError,
    > {
        let contains_transaction = self
            .pending_block
            .transactions()
            .iter()
            .any(|tx| tx.transaction_hash() == transaction_hash);

        if contains_transaction {
            Ok(Some(self.pending_block.clone()))
        } else {
            self.blockchain.block_by_transaction_hash(transaction_hash)
        }
    }

    fn chain_id(&self) -> u64 {
        self.blockchain.chain_id()
    }

    fn last_block(
        &self,
    ) -> Result<Arc<dyn SyncBlock<ChainSpecT, Error = Self::BlockchainError>>, Self::BlockchainError>
    {
        Ok(self.pending_block.clone())
    }

    fn last_block_number(&self) -> u64 {
        self.pending_block.header().number
    }

    fn logs(
        &self,
        _from_block: u64,
        _to_block: u64,
        _addresses: &HashSet<edr_eth::Address>,
        _normalized_topics: &[Option<Vec<B256>>],
    ) -> Result<Vec<edr_eth::log::FilterLog>, Self::BlockchainError> {
        panic!("Retrieving logs from a pending blockchain is not supported.");
    }

    fn network_id(&self) -> u64 {
        self.blockchain.network_id()
    }

    fn receipt_by_transaction_hash(
        &self,
        transaction_hash: &B256,
    ) -> Result<Option<Arc<BlockReceipt<ChainSpecT>>>, Self::BlockchainError> {
        let pending_receipt = self
            .pending_block
            .fetch_transaction_receipts()?
            .into_iter()
            .find(|receipt| receipt.transaction_hash == *transaction_hash);

        if let Some(pending_receipt) = pending_receipt {
            Ok(Some(pending_receipt))
        } else {
            self.blockchain
                .receipt_by_transaction_hash(transaction_hash)
        }
    }

    fn spec_at_block_number(
        &self,
        block_number: u64,
    ) -> Result<ChainSpecT::Hardfork, Self::BlockchainError> {
        if block_number == self.pending_block.header().number {
            Ok(self.blockchain.hardfork())
        } else {
            self.blockchain.spec_at_block_number(block_number)
        }
    }

    fn hardfork(&self) -> ChainSpecT::Hardfork {
        self.blockchain.hardfork()
    }

    fn state_at_block_number(
        &self,
        block_number: u64,
        state_overrides: &BTreeMap<u64, StateOverride>,
    ) -> Result<Box<dyn SyncState<Self::StateError>>, Self::BlockchainError> {
        if block_number == self.pending_block.header().number {
            assert!(
                state_overrides.get(&block_number).is_none(),
                "State overrides are not supported for a pending block."
            );

            let mut state = self
                .blockchain
                .state_at_block_number(block_number - 1, state_overrides)?;

            state.commit(self.pending_state_diff.as_inner().clone());

            Ok(state)
        } else {
            self.blockchain
                .state_at_block_number(block_number, state_overrides)
        }
    }

    fn total_difficulty_by_hash(&self, hash: &B256) -> Result<Option<U256>, Self::BlockchainError> {
        if hash == self.pending_block.block_hash() {
            let previous_total_difficulty = self
                .blockchain
                .total_difficulty_by_hash(&self.pending_block.header().parent_hash)?
                .expect("At least one block should exist before the pending block.");

            Ok(Some(
                previous_total_difficulty + self.pending_block.header().difficulty,
            ))
        } else {
            self.blockchain.total_difficulty_by_hash(hash)
        }
    }
}

impl<'blockchain, ChainSpecT: SyncRuntimeSpec> BlockchainMut<ChainSpecT>
    for BlockchainWithPending<'blockchain, ChainSpecT>
{
    type Error = BlockchainError<ChainSpecT>;

    fn insert_block(
        &mut self,
        _block: EthLocalBlock<ChainSpecT>,
        _state_diff: StateDiff,
    ) -> Result<BlockAndTotalDifficulty<ChainSpecT, Self::Error>, Self::Error> {
        panic!("Inserting blocks into a pending blockchain is not supported.");
    }

    fn reserve_blocks(&mut self, _additional: u64, _interval: u64) -> Result<(), Self::Error> {
        panic!("Reserving blocks in a pending blockchain is not supported.");
    }

    fn revert_to_block(&mut self, _block_number: u64) -> Result<(), Self::Error> {
        panic!("Reverting blocks in a pending blockchain is not supported.");
    }
}

impl<'blockchain, ChainSpecT: SyncRuntimeSpec> BlockHash
    for BlockchainWithPending<'blockchain, ChainSpecT>
{
    type Error = BlockchainError<ChainSpecT>;

    fn block_hash_by_number(&self, block_number: u64) -> Result<B256, Self::Error> {
        if block_number == self.pending_block.header().number {
            Ok(*self.pending_block.block_hash())
        } else {
            self.blockchain.block_hash_by_number(block_number)
        }
    }
}
