//! Synchronous blockchain traits and implementations.

use core::fmt::Debug;

use crate::{BlockHash, Blockchain, BlockchainMut};

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
