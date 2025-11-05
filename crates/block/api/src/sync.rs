//! Synchronous Ethereum block traits and implementations.

use edr_receipt::ReceiptTrait;

use crate::EthBlock;

/// Trait that meets all requirements for a synchronous Ethereum block.
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
