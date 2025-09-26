use std::{fmt::Debug, marker::PhantomData, sync::Arc};

use alloy_eips::eip7840::BlobParams;
use edr_block_header::{
    calculate_next_base_fee_per_gas, BlobGas, BlockConfig, BlockHeader, PartialHeader,
};
use edr_chain_l1::L1ChainSpec;
use edr_eip1559::BaseFeeParams;
use edr_evm_spec::{
    BlobExcessGasAndPrice, ChainHardfork, ChainSpec, EthHeaderConstants, EvmSpecId,
    EvmTransactionValidationError, ExecutableTransaction, TransactionValidation,
};
use edr_primitives::{Bytes, B256, U256};
use edr_receipt::{
    log::{ExecutionLog, FilterLog},
    BlockReceipt, ExecutionReceipt, MapReceiptLogs, ReceiptFactory, ReceiptTrait,
};
use edr_rpc_spec::{RpcSpec, RpcTypeFrom};
use edr_transaction::TransactionType;
use edr_utils::types::TypeConstructor;
use revm::{inspector::NoOpInspector, ExecuteEvm, InspectEvm, Inspector};
pub use revm_context_interface::ContextTr as ContextTrait;
use revm_handler::{instructions::EthInstructions, EthFrame, PrecompileProvider};
use revm_interpreter::{interpreter::EthInterpreter, InterpreterResult};

use crate::{
    block::{transaction::TransactionAndBlockForChainSpec, LocalCreationError},
    config::CfgEnv,
    evm::Evm,
    hardfork::{self, ChainConfig},
    journal::Journal,
    precompile::EthPrecompiles,
    receipt::{self, ExecutionReceiptBuilder},
    result::{EVMErrorForChain, ExecutionResult},
    state::{Database, DatabaseComponentError, EvmState, StateDiff},
    transaction::{remote::EthRpcTransaction, TransactionError, TransactionErrorForChainSpec},
    Block, BlockBuilder, BlockReceipts, EmptyBlock, EthBlockBuilder, EthBlockData,
    EthBlockReceiptFactory, EthLocalBlock, EthLocalBlockForChainSpec, EthRpcBlock,
    GenesisBlockOptions, LocalBlock, RemoteBlock, RemoteBlockConversionError, SyncBlock,
};

/// Ethereum L1 extra data for genesis blocks.
pub const EXTRA_DATA: &[u8] = b"\x12\x34";

/// Helper type for a chain-specific [`revm::Context`].
pub type ContextForChainSpec<ChainSpecT, DatabaseT> = revm::Context<
    <ChainSpecT as ChainSpec>::BlockEnv,
    <ChainSpecT as ChainSpec>::SignedTransaction,
    CfgEnv<<ChainSpecT as ChainHardfork>::Hardfork>,
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

/// Trait for constructing a chain-specific genesis block.
pub trait GenesisBlockFactory: ChainHardfork {
    /// The error type for genesis block creation.
    type CreationError: std::error::Error;

    /// The local block type.
    type LocalBlock;

    /// Constructs a genesis block for the given chain spec.
    fn genesis_block(
        genesis_diff: StateDiff,
        block_config: BlockConfig<'_, Self::Hardfork>,
        options: GenesisBlockOptions<Self::Hardfork>,
    ) -> Result<Self::LocalBlock, Self::CreationError>;
}

/// A supertrait for [`GenesisBlockFactory`] that is safe to send between
/// threads.
pub trait SyncGenesisBlockFactory:
    GenesisBlockFactory<CreationError: Send + Sync> + Sync + Send
{
}

impl<FactoryT> SyncGenesisBlockFactory for FactoryT where
    FactoryT: GenesisBlockFactory<CreationError: Send + Sync> + Sync + Send
{
}

impl GenesisBlockFactory for L1ChainSpec {
    type CreationError = LocalCreationError;

    type LocalBlock = <Self as RuntimeSpec>::LocalBlock;

    fn genesis_block(
        genesis_diff: StateDiff,
        block_config: BlockConfig<'_, Self::Hardfork>,
        mut options: GenesisBlockOptions<Self::Hardfork>,
    ) -> Result<Self::LocalBlock, Self::CreationError> {
        // If no option is provided, use the default extra data for L1 Ethereum.
        options.extra_data = Some(
            options
                .extra_data
                .unwrap_or(Bytes::copy_from_slice(EXTRA_DATA)),
        );

        EthLocalBlockForChainSpec::<Self>::with_genesis_state::<Self>(
            genesis_diff,
            block_config,
            options,
        )
    }
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

/// Returns the corresponding base fee params configured for the given chain ID.
/// If it's not defined in the defined chain specification it fallbacks to the
/// chain spec default.
pub fn base_fee_params_for<ChainSpecT: RuntimeSpec>(
    chain_id: u64,
) -> &'static BaseFeeParams<ChainSpecT::Hardfork> {
    ChainSpecT::chain_config(chain_id).and_then( |config| 
        config.base_fee_params.as_ref()
    ).unwrap_or(&ChainSpecT::default_base_fee_params())
}
/// A trait for constructing a (partial) block header into an EVM block.
pub trait BlockEnvConstructor<HeaderT>: ChainSpec {
    /// Converts the instance into an EVM block.
    fn new_block_env(header: &HeaderT, hardfork: EvmSpecId) -> Self::BlockEnv;
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
        EthFrame<EthInterpreter>,
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
    type RpcReceiptConversionError = edr_chain_l1::rpc::receipt::ConversionError;
    type RpcTransactionConversionError = edr_chain_l1::rpc::transaction::ConversionError;

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
            EvmTransactionValidationError::LackOfFundForMaxFee { fee, balance } => {
                TransactionError::LackOfFundForMaxFee { fee, balance }
            }
            remainder => TransactionError::InvalidTransaction(remainder),
        }
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
        Evm::new_with_inspector(
            context,
            inspector,
            EthInstructions::default(),
            precompile_provider,
        )
    }

    fn next_base_fee_per_gas(
        header: &BlockHeader,
        chain_id: u64,
        hardfork: Self::Hardfork,
        base_fee_params_overrides: Option<&BaseFeeParams<Self::Hardfork>>,
    ) -> u128 {
        calculate_next_base_fee_per_gas(
            header,
            base_fee_params_overrides.unwrap_or(base_fee_params_for::<Self>(chain_id)),
            hardfork,
        )
    }

    fn chain_config(chain_id: u64) -> Option<&'static ChainConfig<Self::Hardfork>> {
        hardfork::l1::chain_config(chain_id)
    }

    fn default_base_fee_params() -> &'static BaseFeeParams<Self::Hardfork> {
        hardfork::l1::default_base_fee_params()
    }
}

impl BlockEnvConstructor<PartialHeader> for L1ChainSpec {
    fn new_block_env(header: &PartialHeader, hardfork: EvmSpecId) -> Self::BlockEnv {
        edr_chain_l1::BlockEnv {
            number: U256::from(header.number),
            beneficiary: header.beneficiary,
            timestamp: U256::from(header.timestamp),
            difficulty: header.difficulty,
            basefee: header.base_fee.map_or(0u64, |base_fee| {
                base_fee.try_into().expect("base fee is too large")
            }),
            gas_limit: header.gas_limit,
            prevrandao: if hardfork >= EvmSpecId::MERGE {
                Some(header.mix_hash)
            } else {
                None
            },
            blob_excess_gas_and_price: header.blob_gas.as_ref().map(
                |BlobGas { excess_gas, .. }| {
                    let blob_params = if hardfork >= EvmSpecId::PRAGUE {
                        BlobParams::prague()
                    } else {
                        BlobParams::cancun()
                    };

                    BlobExcessGasAndPrice::new(
                        *excess_gas,
                        blob_params
                            .update_fraction
                            .try_into()
                            .expect("blob update fraction is too large"),
                    )
                },
            ),
        }
    }
}

impl BlockEnvConstructor<BlockHeader> for L1ChainSpec {
    fn new_block_env(header: &BlockHeader, hardfork: EvmSpecId) -> Self::BlockEnv {
        edr_chain_l1::BlockEnv {
            number: U256::from(header.number),
            beneficiary: header.beneficiary,
            timestamp: U256::from(header.timestamp),
            difficulty: header.difficulty,
            basefee: header.base_fee_per_gas.map_or(0u64, |base_fee| {
                base_fee.try_into().expect("base fee is too large")
            }),
            gas_limit: header.gas_limit,
            prevrandao: if hardfork >= EvmSpecId::MERGE {
                Some(header.mix_hash)
            } else {
                None
            },
            blob_excess_gas_and_price: header.blob_gas.as_ref().map(
                |BlobGas { excess_gas, .. }| {
                    let blob_params = if hardfork >= EvmSpecId::PRAGUE {
                        BlobParams::prague()
                    } else {
                        BlobParams::cancun()
                    };

                    BlobExcessGasAndPrice::new(
                        *excess_gas,
                        blob_params
                            .update_fraction
                            .try_into()
                            .expect("blob update fraction is too large"),
                    )
                },
            ),
        }
    }
}

#[cfg(test)]
mod l1_chain_spec_tests {
    use edr_block_header::{BlobGas, BlockHeader};
    use edr_primitives::{Address, Bloom, Bytes, B256, B64, U256};

    use crate::spec::{BlockEnvConstructor as _, L1ChainSpec};

    fn build_block_header(blob_gas: Option<BlobGas>) -> BlockHeader {
        BlockHeader {
            parent_hash: B256::default(),
            ommers_hash: B256::default(),
            beneficiary: Address::default(),
            state_root: B256::default(),
            transactions_root: B256::default(),
            receipts_root: B256::default(),
            logs_bloom: Bloom::default(),
            difficulty: U256::default(),
            number: 124,
            gas_limit: u64::default(),
            gas_used: 1337,
            timestamp: 0,
            extra_data: Bytes::default(),
            mix_hash: B256::default(),
            nonce: B64::from(99u64),
            base_fee_per_gas: None,
            withdrawals_root: None,
            blob_gas,
            parent_beacon_block_root: None,
            requests_hash: Some(B256::random()),
        }
    }

    #[test]
    fn l1_block_constructor_should_not_default_excess_blob_gas_for_cancun() {
        let header = build_block_header(None); // No blob gas information

        let block = L1ChainSpec::new_block_env(&header, edr_chain_l1::Hardfork::CANCUN);
        assert_eq!(block.blob_excess_gas_and_price, None);
    }

    #[test]
    fn l1_block_constructor_should_not_default_excess_blob_gas_before_cancun() {
        let header = build_block_header(None); // No blob gas information

        let block = L1ChainSpec::new_block_env(&header, edr_chain_l1::Hardfork::SHANGHAI);
        assert_eq!(block.blob_excess_gas_and_price, None);
    }

    #[test]
    fn l1_block_constructor_should_not_default_excess_blob_gas_after_cancun() {
        let header = build_block_header(None); // No blob gas information

        let block = L1ChainSpec::new_block_env(&header, edr_chain_l1::Hardfork::PRAGUE);
        assert_eq!(block.blob_excess_gas_and_price, None);
    }

    #[test]
    fn l1_block_constructor_should_use_existing_excess_blob_gas() {
        let excess_gas = 0x80000u64;
        let blob_gas = BlobGas {
            excess_gas,
            gas_used: 0x80000u64,
        };
        let header = build_block_header(Some(blob_gas)); // blob gas present

        let block = L1ChainSpec::new_block_env(&header, edr_chain_l1::Hardfork::CANCUN);

        let blob_excess_gas = block
            .blob_excess_gas_and_price
            .expect("Blob excess gas should be set");
        assert_eq!(blob_excess_gas.excess_blob_gas, excess_gas);
    }
}
