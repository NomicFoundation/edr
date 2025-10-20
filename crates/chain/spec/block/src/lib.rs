use std::sync::Arc;

use edr_block_api::{Block, BlockReceipts, EmptyBlock, LocalBlock};
use edr_block_builder_api::BlockBuilder;
use edr_block_header::{BlockEnvConstructor, BlockEnvForHardfork};
use edr_evm_spec::{BlockEnvTrait, EvmChainSpec};
use edr_receipt_spec::ReceiptChainSpec;

/// Trait for specifying the types representing and building a chain's blocks.
pub trait BlockChainSpec:
    EvmChainSpec
    + ReceiptChainSpec<Hardfork: Send + Sync, Receipt: Send + Sync, SignedTransaction: Send + Sync>
{
    /// Type representing block trait objects.
    type Block: Block<Self::SignedTransaction> + BlockReceipts<Arc<Self::Receipt>> + ?Sized;

    /// Type representing a block environment; i.e. the header of the block
    /// (being mined) and its hardfork.
    type BlockEnv<'header, BlockHeaderT>: BlockEnvConstructor<Self::Hardfork, &'header BlockHeaderT>
        + BlockEnvTrait
    where
        BlockHeaderT: 'header + BlockEnvForHardfork<Self::Hardfork>;

    /// Type representing a block builder.
    type BlockBuilder<
        'builder,
        BlockchainErrorT: 'builder + std::error::Error + Send,
        StateErrorT: 'builder + std::error::Error + Send
    >: BlockBuilder<
        'builder,
        Self::Receipt,
        Self::Block,
        Self,
        BlockchainError = BlockchainErrorT,
        LocalBlock = Self::LocalBlock,
        StateError = StateErrorT>;

    /// Type representing a locally mined block.
    type LocalBlock: Block<Self::SignedTransaction>
        + BlockReceipts<Arc<Self::Receipt>>
        + EmptyBlock<Self::Hardfork>
        + LocalBlock<Arc<Self::Receipt>>;
}
