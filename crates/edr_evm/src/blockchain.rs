use std::{collections::BTreeMap, fmt::Debug, ops::Bound::Included};

use edr_block_api::Block;
use edr_block_storage::ReservableSparseBlockStorage;
use edr_blockchain_api::{BlockHash, Blockchain, BlockchainMut};
use edr_evm_spec::{ChainHardfork, EvmSpecId, TransactionValidation};
use edr_primitives::B256;
use edr_receipt::ReceiptTrait;
use edr_state_api::{StateCommit, StateDiff, StateOverride};

// pub use self::{
//     forked::{CreationError as ForkedCreationError, ForkedBlockchain, ForkedBlockchainError},
//     local::{InvalidGenesisBlock, LocalBlockchain},
// };
use crate::{hardfork::Activations, spec::SyncRuntimeSpec};

/// Helper type for a chain-specific [`BlockchainError`].
pub type BlockchainErrorForChainSpec<ChainSpecT> = BlockchainError<
    <ChainSpecT as RuntimeSpec>::RpcBlockConversionError,
    <ChainSpecT as ChainHardfork>::Hardfork,
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
    Insert(#[from] edr_block_storage::InsertBlockError),
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

/// Trait that meets all requirements for a synchronous blockchain.
pub trait SyncBlockchain<
    BlockReceiptT: Send + Sync,
    BlockT: ?Sized,
    BlockchainErrorT: Debug + Send,
    HardforkT: Send + Sync,
    LocalBlockT: Send + Sync,
    SignedTransactionT: Send + Sync,
    StateErrorT,
>:
    Blockchain<
        BlockT,
        BlockReceiptT,
        HardforkT,
        BlockchainError = BlockchainErrorT,
        StateError = StateErrorT,
    > + BlockchainMut<BlockT, LocalBlockT, SignedTransactionT, Error = BlockchainErrorT>
    + BlockHash<Error = BlockchainErrorT>
    + Send
    + Sync
    + Debug
{
}

impl<
        BlockReceiptT: Send + Sync,
        BlockT: ?Sized,
        BlockchainErrorT: Debug + Send,
        BlockchainT: Blockchain<
                BlockT,
                BlockReceiptT,
                HardforkT,
                BlockchainError = BlockchainErrorT,
                StateError = StateErrorT,
            > + BlockchainMut<BlockT, LocalBlockT, SignedTransactionT, Error = BlockchainErrorT>
            + BlockHash<Error = BlockchainErrorT>
            + Send
            + Sync
            + Debug,
        HardforkT: Send + Sync,
        LocalBlockT: Send + Sync,
        SignedTransactionT: TransactionValidation<ValidationError: Send + Sync> + Send + Sync,
        StateErrorT,
    >
    SyncBlockchain<
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
        StateErrorT,
    > for BlockchainT
{
}

/// Helper type for a chain-specific [`SyncBlockchain`].
pub trait SyncBlockchainForChainSpec<
    BlockchainErrorT: Debug + Send,
    ChainSpecT: SyncRuntimeSpec,
    StateErrorT,
>:
    SyncBlockchain<
    ChainSpecT::BlockReceipt,
    ChainSpecT::Block,
    BlockchainErrorT,
    ChainSpecT::Hardfork,
    ChainSpecT::LocalBlock,
    ChainSpecT::SignedTransaction,
    StateErrorT,
>
{
}

impl<
        BlockchainErrorT: Debug + Send,
        BlockchainT: SyncBlockchain<
            ChainSpecT::BlockReceipt,
            ChainSpecT::Block,
            BlockchainErrorT,
            ChainSpecT::Hardfork,
            ChainSpecT::LocalBlock,
            ChainSpecT::SignedTransaction,
            StateErrorT,
        >,
        ChainSpecT: SyncRuntimeSpec,
        StateErrorT,
    > SyncBlockchainForChainSpec<BlockchainErrorT, ChainSpecT, StateErrorT> for BlockchainT
{
}

fn compute_state_at_block<
    BlockReceiptT: Clone + ReceiptTrait,
    BlockT: Block<SignedTransactionT> + Clone,
    HardforkT: Clone,
    SignedTransactionT,
>(
    state: &mut dyn StateCommit,
    local_storage: &ReservableSparseBlockStorage<
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

    if spec_id.into() >= EvmSpecId::SHANGHAI && next_header.withdrawals_root.is_none() {
        return Err(BlockchainError::MissingWithdrawals);
    }

    Ok(())
}
