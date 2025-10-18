//! Synchronous blockchain traits and implementations.

use core::fmt::Debug;

use edr_chain_spec::TransactionValidation;

use crate::Blockchain;

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
        BlockReceiptT,
        BlockT,
        BlockchainErrorT,
        HardforkT,
        LocalBlockT,
        SignedTransactionT,
        StateErrorT,
    > + Send
    + Sync
    + Debug
{
}

impl<
        BlockReceiptT: Send + Sync,
        BlockT: ?Sized,
        BlockchainErrorT: Debug + Send,
        BlockchainT: Blockchain<
                BlockReceiptT,
                BlockT,
                BlockchainErrorT,
                HardforkT,
                LocalBlockT,
                SignedTransactionT,
                StateErrorT,
            > + Send
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
