mod forked;
mod local;
mod remote;
/// Storage data structures for a blockchain
pub mod storage;

use std::{collections::BTreeMap, fmt::Debug, ops::Bound::Included, sync::Arc};

use auto_impl::auto_impl;
use edr_eth::{
    l1, log::FilterLog, receipt::ReceiptTrait, spec::ChainSpec, Address, HashSet, B256, U256,
};

use self::storage::ReservableSparseBlockchainStorage;
pub use self::{
    forked::{CreationError as ForkedCreationError, ForkedBlockchain, ForkedBlockchainError},
    local::{CreationError as LocalCreationError, GenesisBlockOptions, LocalBlockchain},
};
use crate::{
    hardfork::Activations,
    spec::{RuntimeSpec, SyncRuntimeSpec},
    state::{StateCommit, StateDiff, StateOverride, SyncState},
    Block, BlockAndTotalDifficultyForChainSpec,
};

/// Helper type for a chain-specific [`BlockchainError`].
pub type BlockchainErrorForChainSpec<ChainSpecT> = BlockchainError<
    <ChainSpecT as RuntimeSpec>::RpcBlockConversionError,
    <ChainSpecT as ChainSpec>::Hardfork,
    <ChainSpecT as RuntimeSpec>::RpcReceiptConversionError,
>;

/// Combinatorial error for the blockchain API.
#[derive(Debug, thiserror::Error)]
pub enum BlockchainError<BlockConversionErrorT, HardforkT: Debug, ReceiptConversionErrorT> {
    /// Forked blockchain error
    #[error(transparent)]
    Forked(#[from] ForkedBlockchainError<BlockConversionErrorT, ReceiptConversionErrorT>),
    /// An error that occurs when trying to insert a block into storage.
    #[error(transparent)]
    Insert(#[from] storage::InsertError),
    /// Invalid block number
    #[error("Invalid block number: {actual}. Expected: {expected}.")]
    InvalidBlockNumber {
        /// Provided block number
        actual: u64,
        /// Expected block number
        expected: u64,
    },
    /// Invalid parent hash
    #[error("Invalid parent hash: {actual}. Expected: {expected}.")]
    InvalidParentHash {
        /// Provided parent hash
        actual: B256,
        /// Expected parent hash
        expected: B256,
    },
    /// Missing hardfork activation history
    #[error(
        "No known hardfork for execution on historical block {block_number} (relative to fork block number {fork_block_number}) in chain with id {chain_id}. The node was not configured with a hardfork activation history."
    )]
    MissingHardforkActivations {
        /// Block number
        block_number: u64,
        /// Fork block number
        fork_block_number: u64,
        /// Chain id
        chain_id: u64,
    },
    /// Missing withdrawals for post-Shanghai blockchain
    #[error("Missing withdrawals for post-Shanghai blockchain")]
    MissingWithdrawals,
    /// Block number does not exist in blockchain
    #[error("Unknown block number")]
    UnknownBlockNumber,
    /// No hardfork found for block
    #[error(
        "Could not find a hardfork to run for block {block_number}, after having looked for one in the hardfork activation history, which was: {hardfork_activations:?}."
    )]
    UnknownBlockSpec {
        /// Block number
        block_number: u64,
        /// Hardfork activation history
        hardfork_activations: Activations<HardforkT>,
    },
}

/// Trait for retrieving a block's hash by number.
#[auto_impl(&, &mut, Box, Rc, Arc)]
pub trait BlockHash {
    /// The blockchain's error type.
    type Error;

    /// Retrieves the block hash at the provided number.
    fn block_hash_by_number(&self, block_number: u64) -> Result<B256, Self::Error>;
}

/// Trait for implementations of an Ethereum blockchain.
#[auto_impl(&)]
pub trait Blockchain<ChainSpecT>
where
    ChainSpecT: RuntimeSpec,
{
    /// The blockchain's error type
    type BlockchainError;

    /// The state's error type
    type StateError;

    /// Retrieves the block with the provided hash, if it exists.
    #[allow(clippy::type_complexity)]
    fn block_by_hash(
        &self,
        hash: &B256,
    ) -> Result<Option<Arc<ChainSpecT::Block>>, Self::BlockchainError>;

    /// Retrieves the block with the provided number, if it exists.
    #[allow(clippy::type_complexity)]
    fn block_by_number(
        &self,
        number: u64,
    ) -> Result<Option<Arc<ChainSpecT::Block>>, Self::BlockchainError>;

    /// Retrieves the block that contains a transaction with the provided hash,
    /// if it exists.
    #[allow(clippy::type_complexity)]
    fn block_by_transaction_hash(
        &self,
        transaction_hash: &B256,
    ) -> Result<Option<Arc<ChainSpecT::Block>>, Self::BlockchainError>;

    /// Retrieves the instances chain ID.
    fn chain_id(&self) -> u64;

    /// Retrieves the chain ID of the block at the provided number.
    /// The chain ID can be different in fork mode pre- and post-fork block
    /// number.
    fn chain_id_at_block_number(&self, _block_number: u64) -> Result<u64, Self::BlockchainError> {
        // Chain id only depends on the block number in fork mode
        Ok(self.chain_id())
    }

    /// Retrieves the last block in the blockchain.
    fn last_block(&self) -> Result<Arc<ChainSpecT::Block>, Self::BlockchainError>;

    /// Retrieves the last block number in the blockchain.
    fn last_block_number(&self) -> u64;

    /// Retrieves the logs that match the provided filter.
    fn logs(
        &self,
        from_block: u64,
        to_block: u64,
        addresses: &HashSet<Address>,
        normalized_topics: &[Option<Vec<B256>>],
    ) -> Result<Vec<FilterLog>, Self::BlockchainError>;

    /// Retrieves the network ID of the blockchain.
    fn network_id(&self) -> u64;

    /// Retrieves the receipt of the transaction with the provided hash, if it
    /// exists.
    fn receipt_by_transaction_hash(
        &self,
        transaction_hash: &B256,
    ) -> Result<Option<Arc<ChainSpecT::BlockReceipt>>, Self::BlockchainError>;

    /// Retrieves the hardfork specification of the block at the provided
    /// number.
    fn spec_at_block_number(
        &self,
        block_number: u64,
    ) -> Result<ChainSpecT::Hardfork, Self::BlockchainError>;

    /// Retrieves the hardfork specification used for new blocks.
    fn hardfork(&self) -> ChainSpecT::Hardfork;

    /// Retrieves the state at a given block.
    ///
    /// The state overrides are applied after the block they are associated
    /// with. The specified override of a nonce may be ignored to maintain
    /// validity.
    fn state_at_block_number(
        &self,
        block_number: u64,
        // Block number -> state overrides
        state_overrides: &BTreeMap<u64, StateOverride>,
    ) -> Result<Box<dyn SyncState<Self::StateError>>, Self::BlockchainError>;

    /// Retrieves the total difficulty at the block with the provided hash.
    fn total_difficulty_by_hash(&self, hash: &B256) -> Result<Option<U256>, Self::BlockchainError>;
}

/// Trait for implementations of a mutable Ethereum blockchain
pub trait BlockchainMut<ChainSpecT: RuntimeSpec> {
    /// The blockchain's error type
    type Error;

    /// Inserts the provided block into the blockchain, returning a reference to
    /// the inserted block.
    fn insert_block(
        &mut self,
        block: ChainSpecT::LocalBlock,
        state_diff: StateDiff,
    ) -> Result<BlockAndTotalDifficultyForChainSpec<ChainSpecT>, Self::Error>;

    /// Reserves the provided number of blocks, starting from the next block
    /// number.
    fn reserve_blocks(&mut self, additional: u64, interval: u64) -> Result<(), Self::Error>;

    /// Reverts to the block with the provided number, deleting all later
    /// blocks.
    fn revert_to_block(&mut self, block_number: u64) -> Result<(), Self::Error>;
}

/// Trait that meets all requirements for a synchronous blockchain.
pub trait SyncBlockchain<ChainSpecT, BlockchainErrorT, StateErrorT>:
    Blockchain<ChainSpecT, BlockchainError = BlockchainErrorT, StateError = StateErrorT>
    + BlockchainMut<ChainSpecT, Error = BlockchainErrorT>
    + BlockHash<Error = BlockchainErrorT>
    + Send
    + Sync
    + Debug
where
    BlockchainErrorT: Debug + Send,
    ChainSpecT: SyncRuntimeSpec,
{
}

impl<BlockchainT, ChainSpecT, BlockchainErrorT, StateErrorT>
    SyncBlockchain<ChainSpecT, BlockchainErrorT, StateErrorT> for BlockchainT
where
    BlockchainT: Blockchain<ChainSpecT, BlockchainError = BlockchainErrorT, StateError = StateErrorT>
        + BlockchainMut<ChainSpecT, Error = BlockchainErrorT>
        + BlockHash<Error = BlockchainErrorT>
        + Send
        + Sync
        + Debug,
    BlockchainErrorT: Debug + Send,
    ChainSpecT: SyncRuntimeSpec,
{
}

fn compute_state_at_block<
    BlockReceiptT: Clone + ReceiptTrait,
    BlockT: Block<SignedTransactionT> + Clone,
    HardforkT,
    SignedTransactionT,
>(
    state: &mut dyn StateCommit,
    local_storage: &ReservableSparseBlockchainStorage<
        BlockReceiptT,
        BlockT,
        HardforkT,
        SignedTransactionT,
    >,
    first_local_block_number: u64,
    last_local_block_number: u64,
    state_overrides: &BTreeMap<u64, StateOverride>,
) {
    // If we're dealing with a local block, apply their state diffs
    let state_diffs = local_storage
        .state_diffs_until_block(last_local_block_number)
        .unwrap_or_default();

    let mut overriden_state_diffs: BTreeMap<u64, StateDiff> = state_diffs
        .iter()
        .map(|(block_number, state_diff)| (*block_number, state_diff.clone()))
        .collect();

    for (block_number, state_override) in state_overrides.range((
        Included(&first_local_block_number),
        Included(&last_local_block_number),
    )) {
        overriden_state_diffs
            .entry(*block_number)
            .and_modify(|state_diff| {
                state_diff.apply_diff(state_override.diff.as_inner().clone());
            })
            .or_insert_with(|| state_override.diff.clone());
    }

    for (_block_number, state_diff) in overriden_state_diffs {
        state.commit(state_diff.into());
    }
}

/// Validates whether a block is a valid next block.
fn validate_next_block<ChainSpecT: RuntimeSpec>(
    spec_id: ChainSpecT::Hardfork,
    last_block: &dyn Block<ChainSpecT::SignedTransaction>,
    next_block: &dyn Block<ChainSpecT::SignedTransaction>,
) -> Result<(), BlockchainErrorForChainSpec<ChainSpecT>> {
    let last_header = last_block.header();
    let next_header = next_block.header();

    let next_block_number = last_header.number + 1;
    if next_header.number != next_block_number {
        return Err(BlockchainError::InvalidBlockNumber {
            actual: next_header.number,
            expected: next_block_number,
        });
    }

    if next_header.parent_hash != *last_block.block_hash() {
        return Err(BlockchainError::InvalidParentHash {
            actual: next_header.parent_hash,
            expected: *last_block.block_hash(),
        });
    }

    if spec_id.into() >= l1::SpecId::SHANGHAI && next_header.withdrawals_root.is_none() {
        return Err(BlockchainError::MissingWithdrawals);
    }

    Ok(())
}
