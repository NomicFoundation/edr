use std::sync::Arc;

use edr_block_api::{Block, BlockReceipts, EmptyBlock, LocalBlock};
use edr_block_builder_api::BlockBuilder;
use edr_chain_spec::BlockEnvChainSpec;
use edr_evm_spec::EvmChainSpec;
use edr_receipt_spec::ReceiptChainSpec;

/// Trait for specifying the types representing and building a chain's blocks.
pub trait BlockChainSpec:
    BlockEnvChainSpec
    + EvmChainSpec
    + ReceiptChainSpec
{
    /// Type representing block trait objects.
    type Block: ?Sized + Block<Self::SignedTransaction> + BlockReceipts<Arc<Self::Receipt>, Error = Self::FetchReceiptError>;

    /// Type representing a block builder.
    type BlockBuilder<
        'builder,
        BlockchainErrorT: 'builder + std::error::Error,
        StateErrorT: 'builder + std::error::Error 
    >: BlockBuilder<
        'builder,
        Self,
        Self::Receipt,
        Self::Block,
        BlockchainError = BlockchainErrorT,
        LocalBlock = Self::LocalBlock,
        StateError = StateErrorT>;

    /// Type representing errors that can occur when fetching receipts.
    type FetchReceiptError;

    /// Type representing a locally mined block.
    type LocalBlock: Block<Self::SignedTransaction>
        + BlockReceipts<Arc<Self::Receipt>>
        + EmptyBlock<Self::Hardfork>
        + LocalBlock<Arc<Self::Receipt>>;
}
