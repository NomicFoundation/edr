use core::fmt::Debug;

use edr_blockchain_api::{BlockHash, Blockchain, BlockchainMut};
use edr_chain_spec::TransactionValidation;

// pub use self::{
//     forked::{CreationError as ForkedCreationError, ForkedBlockchain, ForkedBlockchainError},
//     local::{InvalidGenesisBlock, LocalBlockchain},
// };
use crate::spec::SyncRuntimeSpec;

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
