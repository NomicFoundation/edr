use std::sync::Arc;

use edr_block_api::{
    Block, BlockReceipts, EmptyBlock, FetchBlockReceipts, GenesisBlockFactory, LocalBlock,
};
use edr_block_builder_api::BlockBuilder;
use edr_block_remote::{FetchRemoteReceiptError, RemoteBlock};
use edr_chain_spec::{BlockEnvChainSpec, ChainSpec, TransactionValidation};
use edr_chain_spec_evm::EvmChainSpec;
use edr_chain_spec_receipt::ReceiptChainSpec;
use edr_chain_spec_rpc::RpcChainSpec;
use edr_utils::CastArcFrom;

/// Trait for specifying the types representing and building a chain's blocks.
pub trait BlockChainSpec:
    BlockEnvChainSpec
    + EvmChainSpec
    + GenesisBlockFactory<
        LocalBlock: Block<Self::SignedTransaction>
                        + BlockReceipts<Arc<Self::Receipt>>
                        + FetchBlockReceipts<Arc<Self::Receipt>>
                        + EmptyBlock<Self::Hardfork>
                        + LocalBlock<Arc<Self::Receipt>>,
    > + ReceiptChainSpec
{
    /// Type representing block trait objects.
    type Block: ?Sized
        + Block<Self::SignedTransaction>
        + CastArcFrom<<Self as GenesisBlockFactory>::LocalBlock>
        + CastArcFrom<
            RemoteBlock<
                <Self as ReceiptChainSpec>::Receipt,
                <Self as BlockChainSpec>::FetchReceiptError,
                Self,
                <Self as RpcChainSpec>::RpcReceipt,
                <Self as RpcChainSpec>::RpcTransaction,
                <Self as ChainSpec>::SignedTransaction,
            >,
        > + FetchBlockReceipts<Arc<Self::Receipt>, Error = Self::FetchReceiptError>;

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
    type FetchReceiptError: std::error::Error + From<
            FetchRemoteReceiptError<
                <<Self as ReceiptChainSpec>::Receipt as TryFrom<
                    <Self as RpcChainSpec>::RpcReceipt,
                >>::Error,
            >,
        >;
}

/// Trait for [`BlockChainSpec`] that meets all requirements for synchronous
/// operations.
pub trait SyncBlockChainSpec:
    BlockChainSpec<
    Block: Send,
    FetchReceiptError: Send + Sync,
    GenesisBlockCreationError: Send + Sync,
    HaltReason: Send,
    Hardfork: Send + Sync,
    LocalBlock: Send + Sync,
    Receipt: Send + Sync,
    SignedTransaction: Send + Sync + TransactionValidation<ValidationError: Send + Sync>,
>
{
}

impl<
        ChainSpecT: BlockChainSpec<
            Block: Send,
            FetchReceiptError: Send + Sync,
            GenesisBlockCreationError: Send + Sync,
            HaltReason: Send,
            Hardfork: Send + Sync,
            LocalBlock: Send + Sync,
            Receipt: Send + Sync,
            SignedTransaction: Send + Sync + TransactionValidation<ValidationError: Send + Sync>,
        >,
    > SyncBlockChainSpec for ChainSpecT
{
}

/// Helper type for a chain-specific [`RemoteBlock`].
pub type RemoteBlockForChainSpec<ChainSpecT> = RemoteBlock<
    <ChainSpecT as ReceiptChainSpec>::Receipt,
    <ChainSpecT as BlockChainSpec>::FetchReceiptError,
    ChainSpecT,
    <ChainSpecT as RpcChainSpec>::RpcReceipt,
    <ChainSpecT as RpcChainSpec>::RpcTransaction,
    <ChainSpecT as ChainSpec>::SignedTransaction,
>;
