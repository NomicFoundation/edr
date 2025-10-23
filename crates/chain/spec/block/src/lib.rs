use std::sync::Arc;

use edr_block_api::{Block, EmptyBlock, FetchBlockReceipts, GenesisBlockFactory, LocalBlock};
use edr_block_builder_api::BlockBuilder;
use edr_chain_spec::BlockEnvChainSpec;
use edr_evm_spec::EvmChainSpec;
use edr_receipt_spec::ReceiptChainSpec;

/// Trait for specifying the types representing and building a chain's blocks.
pub trait BlockChainSpec:
    BlockEnvChainSpec
    + EvmChainSpec
    + GenesisBlockFactory<
        LocalBlock: Block<Self::SignedTransaction>
                        + FetchBlockReceipts<Arc<Self::Receipt>>
                        + EmptyBlock<Self::Hardfork>
                        + LocalBlock<Arc<Self::Receipt>>,
    > + ReceiptChainSpec
{
    /// Type representing block trait objects.
    type Block: ?Sized
        + Block<Self::SignedTransaction>
        + FetchBlockReceipts<Arc<Self::Receipt>, Error = Self::FetchReceiptError>;

    /// Type representing a block builder.
    type BlockBuilder<'builder, BlockchainErrorT: 'builder + std::error::Error>: BlockBuilder<
        'builder,
        Self,
        Self::Receipt,
        Self::Block,
        BlockchainError = BlockchainErrorT,
        LocalBlock = Self::LocalBlock,
    >;

    /// Type representing errors that can occur when fetching receipts.
    type FetchReceiptError: std::error::Error;
}
