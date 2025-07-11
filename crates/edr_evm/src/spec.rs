use std::{fmt::Debug, marker::PhantomData, sync::Arc};

use edr_eth::{
    block::{self, BlobGas, PartialHeader},
    eips::eip4844,
    l1::{self, BlockEnv, L1ChainSpec},
    log::{ExecutionLog, FilterLog},
    receipt::{BlockReceipt, ExecutionReceipt, MapReceiptLogs, ReceiptTrait},
    result::ResultAndState,
    spec::{ChainSpec, EthHeaderConstants},
    B256,
};
use edr_rpc_eth::{spec::RpcSpec, RpcTypeFrom, TransactionConversionError};
use edr_utils::types::TypeConstructor;
use revm::{inspector::NoOpInspector, ExecuteEvm, InspectEvm, Inspector};
pub use revm_context_interface::ContextTr as ContextTrait;
use revm_handler::{instructions::EthInstructions, PrecompileProvider};
use revm_interpreter::{interpreter::EthInterpreter, InterpreterResult};

use crate::{
    block::transaction::TransactionAndBlockForChainSpec,
    config::CfgEnv,
    evm::Evm,
    hardfork::{self, Activations},
    journal::Journal,
    precompile::EthPrecompiles,
    receipt::{self, ExecutionReceiptBuilder, ReceiptFactory},
    result::EVMErrorForChain,
    state::{Database, DatabaseComponentError},
    transaction::{
        remote::EthRpcTransaction, ExecutableTransaction, TransactionError,
        TransactionErrorForChainSpec, TransactionType, TransactionValidation,
    },
    Block, BlockBuilder, BlockReceipts, EmptyBlock, EthBlockBuilder, EthBlockData,
    EthBlockReceiptFactory, EthLocalBlock, EthRpcBlock, LocalBlock, RemoteBlock,
    RemoteBlockConversionError, SyncBlock,
};

/// Helper type for a chain-specific [`revm::Context`].
pub type ContextForChainSpec<ChainSpecT, DatabaseT> = revm::Context<
    <ChainSpecT as ChainSpec>::BlockEnv,
    <ChainSpecT as ChainSpec>::SignedTransaction,
    CfgEnv<<ChainSpecT as ChainSpec>::Hardfork>,
    DatabaseT,
    Journal<DatabaseT>,
    <ChainSpecT as ChainSpec>::Context,
>;

/// Helper type to construct execution receipt types for a chain spec.
pub struct ExecutionReceiptTypeConstructorForChainSpec<ChainSpecT: RpcSpec> {
    phantom: PhantomData<ChainSpecT>,
}

impl<ChainSpecT: RpcSpec> TypeConstructor<ExecutionLog>
    for ExecutionReceiptTypeConstructorForChainSpec<ChainSpecT>
{
    type Type = ChainSpecT::ExecutionReceipt<ExecutionLog>;
}

impl<ChainSpecT: RpcSpec> TypeConstructor<FilterLog>
    for ExecutionReceiptTypeConstructorForChainSpec<ChainSpecT>
{
    type Type = ChainSpecT::ExecutionReceipt<FilterLog>;
}

/// Helper trait to define the bounds for a type constructor of execution
/// receipts.
pub trait ExecutionReceiptTypeConstructorBounds:
    TypeConstructor<
        ExecutionLog,
        Type: MapReceiptLogs<
            ExecutionLog,
            FilterLog,
            <Self as TypeConstructor<FilterLog>>::Type,
        > + ExecutionReceipt<Log = ExecutionLog>,
    > + TypeConstructor<FilterLog, Type: Debug + ExecutionReceipt<Log = FilterLog>>
{
}

impl<TypeConstructorT> ExecutionReceiptTypeConstructorBounds for TypeConstructorT where
    TypeConstructorT: TypeConstructor<
            ExecutionLog,
            Type: MapReceiptLogs<
                ExecutionLog,
                FilterLog,
                <Self as TypeConstructor<FilterLog>>::Type,
            > + ExecutionReceipt<Log = ExecutionLog>,
        > + TypeConstructor<FilterLog, Type: Debug + ExecutionReceipt<Log = FilterLog>>
{
}

/// A trait for defining a chain's associated types.
// Bug: https://github.com/rust-lang/rust-clippy/issues/12927
#[allow(clippy::trait_duplication_in_bounds)]
pub trait RuntimeSpec:
    alloy_rlp::Encodable
    // Defines the chain's internal types like blocks/headers or transactions
    + EthHeaderConstants
    + ChainSpec<
        BlockEnv: BlockEnvConstructor<block::Header> + BlockEnvConstructor<PartialHeader> + Default,
        Hardfork: Debug,
        SignedTransaction: alloy_rlp::Encodable
          + Clone
          + Debug
          + Default
          + PartialEq
          + Eq
          + ExecutableTransaction
          + TransactionType
          + TransactionValidation<ValidationError: From<l1::InvalidTransaction>>,
    >
    // Defines an RPC spec and conversion between RPC <-> EVM types
    + RpcSpec<
        RpcBlock<<Self as RpcSpec>::RpcTransaction>: EthRpcBlock
          + TryInto<EthBlockData<Self>, Error = Self::RpcBlockConversionError>,
        RpcReceipt: Debug
          + RpcTypeFrom<Self::BlockReceipt, Hardfork = Self::Hardfork>,
        RpcTransaction: EthRpcTransaction
          + RpcTypeFrom<TransactionAndBlockForChainSpec<Self>, Hardfork = Self::Hardfork>
          + TryInto<Self::SignedTransaction, Error = Self::RpcTransactionConversionError>,
    >
    + RpcSpec<ExecutionReceipt<FilterLog>: Debug>
    + RpcSpec<
        ExecutionReceipt<ExecutionLog>: alloy_rlp::Encodable
          + MapReceiptLogs<ExecutionLog, FilterLog, Self::ExecutionReceipt<FilterLog>>,
        RpcBlock<B256>: EthRpcBlock,
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
    >: ExecuteEvm<Output = Result<
        ResultAndState<Self::HaltReason>,
        EVMErrorForChain<Self, BlockchainErrorT, StateErrorT>,
    >> + InspectEvm<Inspector = InspectorT, Output = Result<
        ResultAndState<Self::HaltReason>,
        EVMErrorForChain<Self, BlockchainErrorT, StateErrorT>,
    >>;

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

    /// Returns the hardfork activations corresponding to the provided chain ID,
    /// if it is associated with this chain specification.
    fn chain_hardfork_activations(chain_id: u64) -> Option<&'static Activations<Self::Hardfork>>;

    /// Returns the name corresponding to the provided chain ID, if it is
    /// associated with this chain specification.
    fn chain_name(chain_id: u64) -> Option<&'static str>;

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

}

/// A trait for constructing a (partial) block header into an EVM block.
pub trait BlockEnvConstructor<HeaderT> {
    /// Converts the instance into an EVM block.
    fn new_block_env(header: &HeaderT, hardfork: l1::SpecId) -> Self;
}

impl BlockEnvConstructor<PartialHeader> for BlockEnv {
    fn new_block_env(header: &PartialHeader, hardfork: l1::SpecId) -> Self {
        Self {
            number: header.number,
            beneficiary: header.beneficiary,
            timestamp: header.timestamp,
            difficulty: header.difficulty,
            basefee: header.base_fee.map_or(0u64, |base_fee| {
                base_fee.try_into().expect("base fee is too large")
            }),
            gas_limit: header.gas_limit,
            prevrandao: if hardfork >= l1::SpecId::MERGE {
                Some(header.mix_hash)
            } else {
                None
            },
            blob_excess_gas_and_price: header.blob_gas.as_ref().map(
                |BlobGas { excess_gas, .. }| {
                    eip4844::BlobExcessGasAndPrice::new(*excess_gas, hardfork >= l1::SpecId::PRAGUE)
                },
            ),
        }
    }
}

impl BlockEnvConstructor<block::Header> for BlockEnv {
    fn new_block_env(header: &block::Header, hardfork: l1::SpecId) -> Self {
        Self {
            number: header.number,
            beneficiary: header.beneficiary,
            timestamp: header.timestamp,
            difficulty: header.difficulty,
            basefee: header.base_fee_per_gas.map_or(0u64, |base_fee| {
                base_fee.try_into().expect("base fee is too large")
            }),
            gas_limit: header.gas_limit,
            prevrandao: if hardfork >= l1::SpecId::MERGE {
                Some(header.mix_hash)
            } else {
                None
            },
            blob_excess_gas_and_price: header.blob_gas.as_ref().map(
                |BlobGas { excess_gas, .. }| {
                    eip4844::BlobExcessGasAndPrice::new(*excess_gas, hardfork >= l1::SpecId::PRAGUE)
                },
            ),
        }
    }
}

/// A supertrait for [`RuntimeSpec`] that is safe to send between threads.
pub trait SyncRuntimeSpec:
    RuntimeSpec<
        BlockReceipt: Send + Sync,
        ExecutionReceipt<FilterLog>: Send + Sync,
        HaltReason: Send + Sync,
        Hardfork: Send + Sync,
        LocalBlock: Send + Sync,
        RpcBlockConversionError: Send + Sync,
        RpcReceiptConversionError: Send + Sync,
        SignedTransaction: TransactionValidation<ValidationError: Send + Sync> + Send + Sync,
    > + Send
    + Sync
    + 'static
{
}

impl<ChainSpecT> SyncRuntimeSpec for ChainSpecT where
    ChainSpecT: RuntimeSpec<
            BlockReceipt: Send + Sync,
            ExecutionReceipt<FilterLog>: Send + Sync,
            HaltReason: Send + Sync,
            Hardfork: Send + Sync,
            LocalBlock: Send + Sync,
            RpcBlockConversionError: Send + Sync,
            RpcReceiptConversionError: Send + Sync,
            SignedTransaction: TransactionValidation<ValidationError: Send + Sync> + Send + Sync,
        > + Send
        + Sync
        + 'static
{
}

impl RuntimeSpec for L1ChainSpec {
    type Block = dyn SyncBlock<
        Arc<Self::BlockReceipt>,
        Self::SignedTransaction,
        Error = <Self::LocalBlock as BlockReceipts<Arc<Self::BlockReceipt>>>::Error,
    >;

    type BlockBuilder<
        'builder,
        BlockchainErrorT: 'builder + Send + std::error::Error,
        StateErrorT: 'builder + Send + std::error::Error,
    > = EthBlockBuilder<'builder, BlockchainErrorT, Self, StateErrorT>;

    type BlockReceipt = BlockReceipt<Self::ExecutionReceipt<FilterLog>>;
    type BlockReceiptFactory = EthBlockReceiptFactory<Self::ExecutionReceipt<FilterLog>>;

    type Evm<
        BlockchainErrorT,
        DatabaseT: Database<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        InspectorT: Inspector<ContextForChainSpec<Self, DatabaseT>>,
        PrecompileProviderT: PrecompileProvider<ContextForChainSpec<Self, DatabaseT>, Output = InterpreterResult>,
        StateErrorT,
    > = Evm<
        ContextForChainSpec<Self, DatabaseT>,
        InspectorT,
        EthInstructions<EthInterpreter, ContextForChainSpec<Self, DatabaseT>>,
        PrecompileProviderT,
    >;

    type LocalBlock = EthLocalBlock<
        Self::RpcBlockConversionError,
        Self::BlockReceipt,
        ExecutionReceiptTypeConstructorForChainSpec<Self>,
        Self::Hardfork,
        Self::RpcReceiptConversionError,
        Self::SignedTransaction,
    >;

    type PrecompileProvider<
        BlockchainErrorT,
        DatabaseT: Database<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        StateErrorT,
    > = EthPrecompiles;

    type ReceiptBuilder = receipt::Builder;
    type RpcBlockConversionError = RemoteBlockConversionError<Self::RpcTransactionConversionError>;
    type RpcReceiptConversionError = edr_rpc_eth::receipt::ConversionError;
    type RpcTransactionConversionError = TransactionConversionError;

    fn cast_local_block(local_block: Arc<Self::LocalBlock>) -> Arc<Self::Block> {
        local_block
    }

    fn cast_remote_block(remote_block: Arc<RemoteBlock<Self>>) -> Arc<Self::Block> {
        remote_block
    }

    fn cast_transaction_error<BlockchainErrorT, StateErrorT>(
        error: <Self::SignedTransaction as TransactionValidation>::ValidationError,
    ) -> TransactionErrorForChainSpec<BlockchainErrorT, Self, StateErrorT> {
        match error {
            l1::InvalidTransaction::LackOfFundForMaxFee { fee, balance } => {
                TransactionError::LackOfFundForMaxFee { fee, balance }
            }
            remainder => TransactionError::InvalidTransaction(remainder),
        }
    }

    fn chain_hardfork_activations(chain_id: u64) -> Option<&'static Activations<Self::Hardfork>> {
        hardfork::l1::chain_hardfork_activations(chain_id)
    }

    fn chain_name(chain_id: u64) -> Option<&'static str> {
        hardfork::l1::chain_name(chain_id)
    }

    fn evm_with_inspector<
        BlockchainErrorT,
        DatabaseT: Database<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        InspectorT: Inspector<ContextForChainSpec<Self, DatabaseT>>,
        PrecompileProviderT: PrecompileProvider<ContextForChainSpec<Self, DatabaseT>, Output = InterpreterResult>,
        StateErrorT,
    >(
        context: ContextForChainSpec<Self, DatabaseT>,
        inspector: InspectorT,
        precompile_provider: PrecompileProviderT,
    ) -> Self::Evm<BlockchainErrorT, DatabaseT, InspectorT, PrecompileProviderT, StateErrorT> {
        Evm {
            ctx: context,
            inspector,
            instruction: EthInstructions::default(),
            precompiles: precompile_provider,
        }
    }
}
