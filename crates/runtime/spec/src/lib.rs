use core::fmt::Debug;
use std::sync::Arc;

use edr_block_api::{Block, BlockReceipts, EmptyBlock, EthBlockData, LocalBlock};
use edr_block_header::{BlockHeader, PartialHeader};
use edr_block_remote::RemoteBlock;
use edr_database_components::{Database, DatabaseComponentError};
use edr_eip1559::BaseFeeParams;
use edr_evm_spec::{
    ChainHardfork, ChainSpec, EthHeaderConstants, EvmSpecId, EvmTransactionValidationError,
    ExecutableTransaction, TransactionValidation,
};
use edr_primitives::B256;
use edr_receipt::{
    log::{ExecutionLog, FilterLog},
    ExecutionReceipt, MapReceiptLogs, ReceiptFactory, ReceiptTrait,
};
use edr_rpc_eth::ChainRpcBlock;
use edr_rpc_spec::{RpcEthBlock, RpcSpec, RpcTransaction, RpcTypeFrom};
use edr_state_api::EvmState;
use edr_transaction::TransactionType;

/// Helper type for a chain-specific [`RemoteBlock`].
pub type RemoteBlockForChainSpec<ChainSpecT> = RemoteBlock<
    <ChainSpecT as RuntimeSpec>::BlockReceipt,
    <ChainSpecT as ChainRpcBlock>::RpcBlock<<ChainSpecT as RpcSpec>::RpcTransaction>,
    <ChainSpecT as RpcSpec>::RpcReceipt,
    <ChainSpecT as RpcSpec>::RpcTransaction,
    <ChainSpecT as ChainSpec>::SignedTransaction,
>;

/// A trait for constructing a (partial) block header into an EVM block.
pub trait BlockEnvConstructor<HeaderT>: ChainSpec {
    /// Converts the instance into an EVM block.
    fn new_block_env(header: &HeaderT, hardfork: EvmSpecId) -> Self::BlockEnv;
}

/// A trait for defining a chain's associated types.
// Bug: https://github.com/rust-lang/rust-clippy/issues/12927
#[allow(clippy::trait_duplication_in_bounds)]
pub trait RuntimeSpec:
    alloy_rlp::Encodable
    // Defines the chain's internal types like blocks/headers or transactions
    + EthHeaderConstants
    + ChainHardfork<Hardfork: Debug>
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
          + RpcTypeFrom<Self::BlockReceipt, Hardfork = Self::Hardfork>,
        RpcTransaction: RpcTransaction
          + RpcTypeFrom<TransactionAndBlockForChainSpec<Self>, Hardfork = Self::Hardfork>
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
        + BlockReceipts<Arc<Self::BlockReceipt>>
        + ?Sized;

    /// Type representing a block builder.
    type BlockBuilder<
        'builder,
        BlockchainErrorT: 'builder + std::error::Error + Send,
        StateErrorT: 'builder + std::error::Error + Send
    >: BlockBuilder<
        'builder,
        Self,
        BlockchainError = BlockchainErrorT,
        StateError = StateErrorT>;

    /// Type representing a transaction's receipt in a block.
    type BlockReceipt: Debug +  ExecutionReceipt<Log = FilterLog> + ReceiptTrait + TryFrom<Self::RpcReceipt, Error = Self::RpcReceiptConversionError>;

    /// Type representing a factory for block receipts.
    type BlockReceiptFactory: ReceiptFactory<
        Self::ExecutionReceipt<FilterLog>,
        Self::Hardfork,
        Self::SignedTransaction,
        Output = Self::BlockReceipt
    >;

    /// Type representing an EVM specification for the provided context and error types.
    type Evm<
        BlockchainErrorT,
        DatabaseT: Database<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        InspectorT: Inspector<ContextForChainSpec<Self, DatabaseT>>,
        PrecompileProviderT: PrecompileProvider<ContextForChainSpec<Self, DatabaseT>, Output = InterpreterResult>,
        StateErrorT,
    >: ExecuteEvm<
        ExecutionResult = ExecutionResult<Self::HaltReason>,
        State = EvmState,
        Error = EVMErrorForChain<Self, BlockchainErrorT, StateErrorT>,
        Tx = Self::SignedTransaction,
    > + InspectEvm<Inspector = InspectorT>;

    /// Type representing a locally mined block.
    type LocalBlock: Block<Self::SignedTransaction> +
        BlockReceipts<Arc<Self::BlockReceipt>> +
        EmptyBlock<Self::Hardfork> +
        LocalBlock<Arc<Self::BlockReceipt>>;

    /// Type representing a precompile provider.
    type PrecompileProvider<
        BlockchainErrorT,
        DatabaseT: Database<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        StateErrorT,
    >: Default + PrecompileProvider<ContextForChainSpec<Self, DatabaseT>, Output = InterpreterResult>;

    /// Type representing a builder that constructs an execution receipt.
    type ReceiptBuilder: ExecutionReceiptBuilder<
        Self::HaltReason,
        Self::Hardfork,
        Self::SignedTransaction,
        Receipt = Self::ExecutionReceipt<ExecutionLog>,
    >;

    /// Type representing an error that occurs when converting an RPC block.
    type RpcBlockConversionError: std::error::Error;

    /// Type representing an error that occurs when converting an RPC receipt.
    type RpcReceiptConversionError: std::error::Error;

    /// Type representing an error that occurs when converting an RPC
    /// transaction.
    type RpcTransactionConversionError: std::error::Error;

    /// Casts an [`Arc`] of the [`Self::LocalBlock`] type into an [`Arc`] of the [`Self::Block`] type.
    fn cast_local_block(local_block: Arc<Self::LocalBlock>) -> Arc<Self::Block>;

    /// Casts an [`Arc`] of the [`RemoteBlock`] type into an [`Arc`] of the [`Self::Block`] type.
    fn cast_remote_block(remote_block: Arc<RemoteBlock<Self>>) -> Arc<Self::Block>;

    /// Casts a transaction validation error into a `TransactionError`.
    ///
    /// This is implemented as an associated function to avoid problems when
    /// implementing type conversions for third-party types.
    fn cast_transaction_error<BlockchainErrorT, StateErrorT>(
        error: <Self::SignedTransaction as TransactionValidation>::ValidationError,
    ) -> TransactionErrorForChainSpec<BlockchainErrorT, Self, StateErrorT>;

    /// Returns the corresponding configuration for the provided chain ID, if it is
    /// associated with this chain specification.
    fn chain_config(chain_id: u64) -> Option<&'static ChainConfig<Self::Hardfork>>;

    /// Returns the default base fee params to fallback to for the given spec
    fn default_base_fee_params() -> &'static BaseFeeParams<Self::Hardfork>;

    /// Constructs an EVM instance with the provided context.
    fn evm<
        BlockchainErrorT,
        DatabaseT: Database<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        PrecompileProviderT: PrecompileProvider<
            ContextForChainSpec<Self, DatabaseT>,
            Output = InterpreterResult
        >,
        StateErrorT,
    >(
        context: ContextForChainSpec<Self, DatabaseT>,
        precompile_provider: PrecompileProviderT,
    ) -> Self::Evm<
        BlockchainErrorT,
        DatabaseT,
        NoOpInspector,
        PrecompileProviderT,
        StateErrorT,
    > {
        Self::evm_with_inspector(
            context,
            NoOpInspector {},
            precompile_provider,
        )
    }

    /// Constructs an EVM instance with the provided context and inspector.
    fn evm_with_inspector<
        BlockchainErrorT,
        DatabaseT: Database<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        InspectorT: Inspector<ContextForChainSpec<Self, DatabaseT>>,
        PrecompileProviderT: PrecompileProvider<
            ContextForChainSpec<Self, DatabaseT>,
            Output = InterpreterResult
        >,
        StateErrorT,
    >(
        context: ContextForChainSpec<Self, DatabaseT>,
        inspector: InspectorT,
        precompile_provider: PrecompileProviderT,
    ) -> Self::Evm<
        BlockchainErrorT,
        DatabaseT,
        InspectorT,
        PrecompileProviderT,
        StateErrorT,
    >;

    /// Returns the `base_fee_per_gas` for the next block.
    fn next_base_fee_per_gas(header: &BlockHeader, chain_id: u64, hardfork: Self::Hardfork, base_fee_params_overrides: Option<&BaseFeeParams<Self::Hardfork>>) -> u128;

}
