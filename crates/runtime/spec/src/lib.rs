use core::fmt::Debug;
use std::sync::Arc;

use edr_block_api::{Block, BlockReceipts, EmptyBlock, EthBlockData, LocalBlock};
use edr_block_header::{BlockHeader, PartialHeader};
use edr_block_remote::RemoteBlock;
use edr_chain_config::ChainConfig;
use edr_chain_spec::{
    BlockEnvTrait, ChainHardfork, ChainSpec, EvmSpecId, EvmTransactionValidationError,
    ExecutableTransaction, TransactionValidation,
};
use edr_eip1559::BaseFeeParams;
use edr_primitives::B256;
use edr_receipt::{
    log::{ExecutionLog, FilterLog},
    MapReceiptLogs,
};
use edr_receipt_spec::ChainReceiptSpec;
use edr_rpc_eth::RpcBlockChainSpec;
use edr_rpc_spec::{RpcEthBlock, RpcSpec, RpcTransaction, RpcTypeFrom};
use edr_transaction::{TransactionAndBlock, TransactionType};

/// Helper type for a chain-specific [`RemoteBlock`].
pub type RemoteBlockForChainSpec<ChainSpecT> = RemoteBlock<
    <ChainSpecT as ChainReceiptSpec>::Receipt,
    ChainSpecT,
    <ChainSpecT as RpcSpec>::RpcReceipt,
    <ChainSpecT as RpcSpec>::RpcTransaction,
    <ChainSpecT as ChainSpec>::SignedTransaction,
>;

/// A trait for constructing a (partial) block header into an EVM block.
pub trait BlockEnvConstructor<HeaderT> {
    type BlockEnv: BlockEnvTrait;

    /// Converts the instance into an EVM block.
    fn new_block_env(header: &HeaderT, hardfork: EvmSpecId) -> Self::BlockEnv;
}

/// A trait for defining a chain's associated types.
// Bug: https://github.com/rust-lang/rust-clippy/issues/12927
#[allow(clippy::trait_duplication_in_bounds)]
pub trait RuntimeSpec:
    ChainReceiptSpec +
    ChainHardfork<Hardfork: Debug>
    + ChainSpec<
        SignedTransaction: alloy_rlp::Encodable
          + Clone
          + Debug
          + Default
          + PartialEq
          + Eq
          + ExecutableTransaction
          + TransactionType
          + TransactionValidation<ValidationError: From<EvmTransactionValidationError>>,
    >
    + BlockEnvConstructor<PartialHeader> + BlockEnvConstructor<BlockHeader>
    // Defines an RPC spec and conversion between RPC <-> EVM types
    + RpcSpec<
        RpcBlock<<Self as RpcSpec>::RpcTransaction>: RpcEthBlock
          + TryInto<EthBlockData<Self>, Error = Self::RpcBlockConversionError>,
        RpcReceipt: Debug
          + RpcTypeFrom<Self::Receipt, Hardfork = Self::Hardfork>,
        RpcTransaction: RpcTransaction
          + RpcTypeFrom<TransactionAndBlock<Arc<Self::Block>, Self::SignedTransaction>, Hardfork = Self::Hardfork>
          + TryInto<Self::SignedTransaction, Error = Self::RpcTransactionConversionError>,
    >
    + RpcSpec<ExecutionReceipt<FilterLog>: Debug>
    + RpcSpec<
        ExecutionReceipt<ExecutionLog>: alloy_rlp::Encodable
          + MapReceiptLogs<ExecutionLog, FilterLog, Self::ExecutionReceipt<FilterLog>>,
        RpcBlock<B256>: RpcEthBlock,
    >
    + Sized
{
    /// Trait for representing block trait objects.
    type Block: Block<Self::SignedTransaction>
        + BlockReceipts<Arc<Self::Receipt>>
        + ?Sized;

    // /// Type representing a block builder.
    // type BlockBuilder<
    //     'builder,
    //     BlockchainErrorT: 'builder + std::error::Error + Send,
    //     StateErrorT: 'builder + std::error::Error + Send
    // >: BlockBuilder<
    //     'builder,
    //     Self,
    //     BlockchainError = BlockchainErrorT,
    //     StateError = StateErrorT>;

    /// Type representing a locally mined block.
    type LocalBlock: Block<Self::SignedTransaction> +
        BlockReceipts<Arc<Self::Receipt>> +
        EmptyBlock<Self::Hardfork> +
        LocalBlock<Arc<Self::Receipt>>;

    /// Type representing an error that occurs when converting an RPC block.
    type RpcBlockConversionError: std::error::Error;

    /// Type representing an error that occurs when converting an RPC receipt.
    type RpcReceiptConversionError: std::error::Error;

    /// Type representing an error that occurs when converting an RPC
    /// transaction.
    type RpcTransactionConversionError: std::error::Error;

    /// Returns the corresponding configuration for the provided chain ID, if it is
    /// associated with this chain specification.
    fn chain_config(chain_id: u64) -> Option<&'static ChainConfig<Self::Hardfork>>;

    /// Returns the default base fee params to fallback to for the given spec
    fn default_base_fee_params() -> &'static BaseFeeParams<Self::Hardfork>;

    /// Returns the `base_fee_per_gas` for the next block.
    fn next_base_fee_per_gas(header: &BlockHeader, chain_id: u64, hardfork: Self::Hardfork, base_fee_params_overrides: Option<&BaseFeeParams<Self::Hardfork>>) -> u128;

}
