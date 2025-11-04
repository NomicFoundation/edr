//! Synchronous blockchain traits and implementations.

use edr_chain_spec::TransactionValidation;

use crate::Blockchain;

/// Trait that meets all requirements for a synchronous blockchain.
pub trait SyncBlockchain<
    BlockReceiptT: Send,
    BlockT: ?Sized,
    BlockchainErrorT: Send,
    HardforkT: Send,
    LocalBlockT: Send,
    SignedTransactionT: Send,
>:
    Blockchain<BlockReceiptT, BlockT, BlockchainErrorT, HardforkT, LocalBlockT, SignedTransactionT>
    + Send
{
}

impl<
        BlockReceiptT: Send,
        BlockT: ?Sized,
        BlockchainErrorT: Send,
        BlockchainT: Blockchain<
                BlockReceiptT,
                BlockT,
                BlockchainErrorT,
                HardforkT,
                LocalBlockT,
                SignedTransactionT,
            > + Send,
        HardforkT: Send,
        LocalBlockT: Send,
        SignedTransactionT: TransactionValidation<ValidationError: Send> + Send,
    >
    SyncBlockchain<
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
    > for BlockchainT
{
}
