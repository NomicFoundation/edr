//! Types for dynamic dispatch of blockchain implementations.

use core::marker::PhantomData;
use std::{collections::BTreeMap, sync::Arc};

use edr_block_api::BlockAndTotalDifficulty;
use edr_eip1559::BaseFeeParams;
use edr_primitives::{Address, HashSet, B256, U256};
use edr_receipt::log::FilterLog;
use edr_state_api::{DynState, StateDiff, StateOverride};

use crate::{
    BlockHashByNumber, Blockchain, BlockchainMetadata, GetBlockchainBlock, GetBlockchainLogs,
    InsertBlock, ReceiptByTransactionHash, ReserveBlocks, RevertToBlock, StateAtBlock,
    TotalDifficultyByBlockHash,
};

/// Wrapper around `Box<dyn std::error::Error` to allow implementation of
/// `std::error::Error`.
// This is required because of:
// <https://stackoverflow.com/questions/65151237/why-doesnt-boxdyn-error-implement-error#65151318>
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct DynBlockchainError(Box<dyn std::error::Error + Send + Sync>);

impl DynBlockchainError {
    /// Constructs a new instance.
    pub fn new<ErrorT: 'static + std::error::Error + Send + Sync>(error: ErrorT) -> Self {
        Self(Box::<dyn std::error::Error + Send + Sync>::from(error))
    }
}

/// Wrapper struct for dynamic dispatch of a blockchain implementation.
///
/// Error types are converted into `Box<dyn std::error::Error>` for dynamic
/// dispatch.
pub struct DynBlockchain<
    BlockReceiptT,
    BlockT: ?Sized,
    BlockchainErrorT: 'static + std::error::Error + Send + Sync,
    BlockchainT: Blockchain<BlockReceiptT, BlockT, BlockchainErrorT, HardforkT, LocalBlockT, SignedTransactionT>,
    HardforkT,
    LocalBlockT,
    SignedTransactionT,
> {
    inner: BlockchainT,
    #[allow(clippy::type_complexity)]
    _phantom: PhantomData<
        fn() -> (
            BlockReceiptT,
            BlockchainErrorT,
            HardforkT,
            LocalBlockT,
            SignedTransactionT,
            // only the last element of a tuple may have a dynamically sized type
            BlockT,
        ),
    >,
}

impl<
        BlockReceiptT,
        BlockT: ?Sized,
        BlockchainErrorT: 'static + std::error::Error + Send + Sync,
        BlockchainT: Blockchain<
            BlockReceiptT,
            BlockT,
            BlockchainErrorT,
            HardforkT,
            LocalBlockT,
            SignedTransactionT,
        >,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    >
    DynBlockchain<
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        BlockchainT,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    >
{
    /// Constructs a new instance.
    pub fn new(inner: BlockchainT) -> Self {
        Self {
            inner,
            _phantom: PhantomData,
        }
    }
}

impl<
        BlockReceiptT,
        BlockT: ?Sized,
        BlockchainErrorT: 'static + std::error::Error + Send + Sync,
        BlockchainT: Blockchain<
            BlockReceiptT,
            BlockT,
            BlockchainErrorT,
            HardforkT,
            LocalBlockT,
            SignedTransactionT,
        >,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    > BlockHashByNumber
    for DynBlockchain<
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        BlockchainT,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    >
{
    type Error = DynBlockchainError;

    fn block_hash_by_number(&self, block_number: u64) -> Result<B256, Self::Error> {
        self.inner
            .block_hash_by_number(block_number)
            .map_err(DynBlockchainError::new)
    }
}

impl<
        BlockReceiptT,
        BlockT: ?Sized,
        BlockchainErrorT: 'static + std::error::Error + Send + Sync,
        BlockchainT: Blockchain<
            BlockReceiptT,
            BlockT,
            BlockchainErrorT,
            HardforkT,
            LocalBlockT,
            SignedTransactionT,
        >,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    > BlockchainMetadata<HardforkT>
    for DynBlockchain<
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        BlockchainT,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    >
{
    type Error = DynBlockchainError;

    fn base_fee_params(&self) -> &BaseFeeParams<HardforkT> {
        self.inner.base_fee_params()
    }

    fn chain_id(&self) -> u64 {
        self.inner.chain_id()
    }

    fn chain_id_at_block_number(&self, block_number: u64) -> Result<u64, Self::Error> {
        self.inner
            .chain_id_at_block_number(block_number)
            .map_err(DynBlockchainError::new)
    }

    fn spec_at_block_number(&self, block_number: u64) -> Result<HardforkT, Self::Error> {
        self.inner
            .spec_at_block_number(block_number)
            .map_err(DynBlockchainError::new)
    }

    fn hardfork(&self) -> HardforkT {
        self.inner.hardfork()
    }

    fn last_block_number(&self) -> u64 {
        self.inner.last_block_number()
    }

    fn min_ethash_difficulty(&self) -> u64 {
        self.inner.min_ethash_difficulty()
    }

    fn network_id(&self) -> u64 {
        self.inner.network_id()
    }
}

impl<
        BlockReceiptT,
        BlockT: ?Sized,
        BlockchainErrorT: 'static + std::error::Error + Send + Sync,
        BlockchainT: Blockchain<
            BlockReceiptT,
            BlockT,
            BlockchainErrorT,
            HardforkT,
            LocalBlockT,
            SignedTransactionT,
        >,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    > GetBlockchainBlock<BlockT, HardforkT>
    for DynBlockchain<
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        BlockchainT,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    >
{
    type Error = DynBlockchainError;

    fn block_by_hash(&self, hash: &B256) -> Result<Option<Arc<BlockT>>, Self::Error> {
        self.inner
            .block_by_hash(hash)
            .map_err(DynBlockchainError::new)
    }

    fn block_by_number(&self, block_number: u64) -> Result<Option<Arc<BlockT>>, Self::Error> {
        self.inner
            .block_by_number(block_number)
            .map_err(DynBlockchainError::new)
    }

    fn block_by_transaction_hash(
        &self,
        transaction_hash: &B256,
    ) -> Result<Option<Arc<BlockT>>, Self::Error> {
        self.inner
            .block_by_transaction_hash(transaction_hash)
            .map_err(DynBlockchainError::new)
    }

    fn last_block(&self) -> Result<Arc<BlockT>, Self::Error> {
        self.inner.last_block().map_err(DynBlockchainError::new)
    }
}

impl<
        BlockReceiptT,
        BlockT: ?Sized,
        BlockchainErrorT: 'static + std::error::Error + Send + Sync,
        BlockchainT: Blockchain<
            BlockReceiptT,
            BlockT,
            BlockchainErrorT,
            HardforkT,
            LocalBlockT,
            SignedTransactionT,
        >,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    > GetBlockchainLogs
    for DynBlockchain<
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        BlockchainT,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    >
{
    type Error = DynBlockchainError;

    fn logs(
        &self,
        from_block: u64,
        to_block: u64,
        addresses: &HashSet<Address>,
        normalized_topics: &[Option<Vec<B256>>],
    ) -> Result<Vec<FilterLog>, Self::Error> {
        self.inner
            .logs(from_block, to_block, addresses, normalized_topics)
            .map_err(DynBlockchainError::new)
    }
}

impl<
        BlockReceiptT,
        BlockT: ?Sized,
        BlockchainErrorT: 'static + std::error::Error + Send + Sync,
        BlockchainT: Blockchain<
            BlockReceiptT,
            BlockT,
            BlockchainErrorT,
            HardforkT,
            LocalBlockT,
            SignedTransactionT,
        >,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    > InsertBlock<BlockT, LocalBlockT, SignedTransactionT>
    for DynBlockchain<
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        BlockchainT,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    >
{
    type Error = DynBlockchainError;

    fn insert_block(
        &mut self,
        block: LocalBlockT,
        state_diff: StateDiff,
    ) -> Result<BlockAndTotalDifficulty<Arc<BlockT>, SignedTransactionT>, Self::Error> {
        self.inner
            .insert_block(block, state_diff)
            .map_err(DynBlockchainError::new)
    }
}

impl<
        BlockReceiptT,
        BlockT: ?Sized,
        BlockchainErrorT: 'static + std::error::Error + Send + Sync,
        BlockchainT: Blockchain<
            BlockReceiptT,
            BlockT,
            BlockchainErrorT,
            HardforkT,
            LocalBlockT,
            SignedTransactionT,
        >,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    > ReceiptByTransactionHash<BlockReceiptT>
    for DynBlockchain<
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        BlockchainT,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    >
{
    type Error = DynBlockchainError;

    fn receipt_by_transaction_hash(
        &self,
        transaction_hash: &B256,
    ) -> Result<Option<Arc<BlockReceiptT>>, Self::Error> {
        self.inner
            .receipt_by_transaction_hash(transaction_hash)
            .map_err(DynBlockchainError::new)
    }
}

impl<
        BlockReceiptT,
        BlockT: ?Sized,
        BlockchainErrorT: 'static + std::error::Error + Send + Sync,
        BlockchainT: Blockchain<
            BlockReceiptT,
            BlockT,
            BlockchainErrorT,
            HardforkT,
            LocalBlockT,
            SignedTransactionT,
        >,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    > ReserveBlocks
    for DynBlockchain<
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        BlockchainT,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    >
{
    type Error = DynBlockchainError;

    fn reserve_blocks(&mut self, additional: u64, interval: u64) -> Result<(), Self::Error> {
        self.inner
            .reserve_blocks(additional, interval)
            .map_err(DynBlockchainError::new)
    }
}

impl<
        BlockReceiptT,
        BlockT: ?Sized,
        BlockchainErrorT: 'static + std::error::Error + Send + Sync,
        BlockchainT: Blockchain<
            BlockReceiptT,
            BlockT,
            BlockchainErrorT,
            HardforkT,
            LocalBlockT,
            SignedTransactionT,
        >,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    > RevertToBlock
    for DynBlockchain<
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        BlockchainT,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    >
{
    type Error = DynBlockchainError;

    fn revert_to_block(&mut self, block_number: u64) -> Result<(), Self::Error> {
        self.inner
            .revert_to_block(block_number)
            .map_err(DynBlockchainError::new)
    }
}

impl<
        BlockReceiptT,
        BlockT: ?Sized,
        BlockchainErrorT: 'static + std::error::Error + Send + Sync,
        BlockchainT: Blockchain<
            BlockReceiptT,
            BlockT,
            BlockchainErrorT,
            HardforkT,
            LocalBlockT,
            SignedTransactionT,
        >,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    > StateAtBlock
    for DynBlockchain<
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        BlockchainT,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    >
{
    type BlockchainError = DynBlockchainError;

    fn state_at_block_number(
        &self,
        block_number: u64,
        state_overrides: &BTreeMap<u64, StateOverride>,
    ) -> Result<Box<dyn DynState>, Self::BlockchainError> {
        self.inner
            .state_at_block_number(block_number, state_overrides)
            .map_err(DynBlockchainError::new)
    }
}

impl<
        BlockReceiptT,
        BlockT: ?Sized,
        BlockchainErrorT: 'static + std::error::Error + Send + Sync,
        BlockchainT: Blockchain<
            BlockReceiptT,
            BlockT,
            BlockchainErrorT,
            HardforkT,
            LocalBlockT,
            SignedTransactionT,
        >,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    > TotalDifficultyByBlockHash
    for DynBlockchain<
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        BlockchainT,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    >
{
    type Error = DynBlockchainError;

    fn total_difficulty_by_hash(&self, hash: &B256) -> Result<Option<U256>, Self::Error> {
        self.inner
            .total_difficulty_by_hash(hash)
            .map_err(DynBlockchainError::new)
    }
}
