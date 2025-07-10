mod builder;
mod local;
mod remote;
/// Block-related transaction types.
pub mod transaction;

use std::{fmt::Debug, marker::PhantomData, sync::Arc};

use auto_impl::auto_impl;
use edr_eth::{
    block::{self, PartialHeader},
    receipt::ReceiptTrait,
    spec::ChainSpec,
    withdrawal::Withdrawal,
    B256, U256,
};
pub use revm_context::BlockEnv;

pub use self::{
    builder::{
        BlockBuilder, BlockBuilderCreationError, BlockBuilderCreationErrorForChainSpec,
        BlockInputs, BlockTransactionError, BlockTransactionErrorForChainSpec, EthBlockBuilder,
        EthBlockReceiptFactory,
    },
    local::{EthLocalBlock, EthLocalBlockForChainSpec},
    remote::{ConversionError as RemoteBlockConversionError, EthRpcBlock, RemoteBlock},
};
use crate::spec::RuntimeSpec;

/// Trait for implementations of an Ethereum block.
#[auto_impl(Arc)]
pub trait Block<SignedTransactionT>: Debug {
    /// Returns the block's hash.
    fn block_hash(&self) -> &B256;

    /// Returns the block's header.
    fn header(&self) -> &block::Header;

    /// Ommer/uncle block hashes.
    fn ommer_hashes(&self) -> &[B256];

    /// The length of the RLP encoding of this block in bytes.
    fn rlp_size(&self) -> u64;

    /// Returns the block's transactions.
    fn transactions(&self) -> &[SignedTransactionT];

    /// Withdrawals
    fn withdrawals(&self) -> Option<&[Withdrawal]>;
}

/// Trait for fetching the receipts of a block's transactions.
#[auto_impl(Arc)]
pub trait BlockReceipts<BlockReceiptT: ReceiptTrait> {
    /// The blockchain error type.
    type Error;

    /// Fetches the receipts of the block's transactions.
    ///
    /// This may block if the receipts are stored remotely.
    fn fetch_transaction_receipts(&self) -> Result<Vec<BlockReceiptT>, Self::Error>;
}

/// Trait for creating an empty block.
pub trait EmptyBlock<HardforkT> {
    /// Constructs an empty block.
    fn empty(hardfork: HardforkT, partial_header: PartialHeader) -> Self;
}

impl<BlockT: EmptyBlock<HardforkT>, HardforkT> EmptyBlock<HardforkT> for Arc<BlockT> {
    fn empty(hardfork: HardforkT, partial_header: PartialHeader) -> Self {
        Arc::new(BlockT::empty(hardfork, partial_header))
    }
}

/// Trait for locally mined blocks.
#[auto_impl(Arc)]
pub trait LocalBlock<BlockReceiptT> {
    /// Returns the receipts of the block's transactions.
    fn transaction_receipts(&self) -> &[BlockReceiptT];
}

/// Trait that meets all requirements for an Ethereum block.
pub trait EthBlock<BlockReceiptT: ReceiptTrait, SignedTransactionT>:
    Block<SignedTransactionT> + BlockReceipts<BlockReceiptT>
{
}

impl<BlockReceiptT, BlockT, SignedTransactionT> EthBlock<BlockReceiptT, SignedTransactionT>
    for BlockT
where
    BlockReceiptT: ReceiptTrait,
    BlockT: Block<SignedTransactionT> + BlockReceipts<BlockReceiptT>,
{
}

/// Trait that meets all requirements for a synchronous block.
pub trait SyncBlock<BlockReceiptT: ReceiptTrait, SignedTransactionT>:
    EthBlock<BlockReceiptT, SignedTransactionT> + Send + Sync
{
}

impl<BlockReceiptT, BlockT, SignedTransactionT> SyncBlock<BlockReceiptT, SignedTransactionT>
    for BlockT
where
    BlockReceiptT: ReceiptTrait,
    BlockT: EthBlock<BlockReceiptT, SignedTransactionT> + Send + Sync,
{
}

/// A type containing the relevant data for an Ethereum block.
pub struct EthBlockData<ChainSpecT: RuntimeSpec> {
    /// The block's header.
    pub header: edr_eth::block::Header,
    /// The block's transactions.
    pub transactions: Vec<ChainSpecT::SignedTransaction>,
    /// The hashes of the block's ommers.
    pub ommer_hashes: Vec<B256>,
    /// The staking withdrawals.
    pub withdrawals: Option<Vec<Withdrawal>>,
    /// The block's hash.
    pub hash: B256,
    /// The length of the RLP encoding of this block in bytes.
    pub rlp_size: u64,
}

/// Helper type for a chain-specific [`BlockAndTotalDifficulty`].
pub type BlockAndTotalDifficultyForChainSpec<ChainSpecT> = BlockAndTotalDifficulty<
    Arc<<ChainSpecT as RuntimeSpec>::Block>,
    <ChainSpecT as ChainSpec>::SignedTransaction,
>;

/// The result returned by requesting a block by number.
#[derive(Clone, Debug)]
pub struct BlockAndTotalDifficulty<BlockT, SignedTransactionT> {
    /// The block
    pub block: BlockT,
    /// The total difficulty with the block
    pub total_difficulty: Option<U256>,
    phantom: PhantomData<SignedTransactionT>,
}

impl<BlockT, SignedTransactionT> BlockAndTotalDifficulty<BlockT, SignedTransactionT> {
    /// Creates a new block and total difficulty.
    pub fn new(block: BlockT, total_difficulty: Option<U256>) -> Self {
        Self {
            block,
            total_difficulty,
            phantom: PhantomData,
        }
    }
}
