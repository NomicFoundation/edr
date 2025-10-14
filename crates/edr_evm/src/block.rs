use edr_block_api::{Block, BlockReceipts};
use edr_receipt::ReceiptTrait;

pub use self::builder::{
    BlockBuilder, BlockBuilderCreationError, BlockInputs, BlockTransactionError,
    BlockTransactionErrorForChainSpec, EthBlockBuilder,
};

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
