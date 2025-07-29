use std::sync::Arc;

use edr_eth::{
    block::{BlobGas, Header, PartialHeader},
    eips::{
        eip1559::BaseFeeParams,
        eip4844::{self},
    },
    l1::{self, BlockEnv, InvalidTransaction, L1ChainSpec},
    log::FilterLog,
    receipt::BlockReceipt,
    spec::{BlockEnvConstructor, ChainHardfork, ChainSpec, EthHeaderConstants},
    transaction::TransactionValidation,
    Bytes,
};
use edr_evm::{
    evm::Evm,
    hardfork::Activations,
    inspector::{Inspector, NoOpInspector},
    interpreter::{EthInstructions, EthInterpreter, InterpreterResult},
    precompile::{EthPrecompiles, PrecompileProvider},
    spec::{
        ContextForChainSpec, ExecutionReceiptTypeConstructorForChainSpec, GenesisBlockFactory,
        RuntimeSpec, EXTRA_DATA,
    },
    state::{Database, DatabaseComponentError},
    transaction::{TransactionError, TransactionErrorForChainSpec},
    BlockReceipts, EthBlockBuilder, EthBlockReceiptFactory, EthLocalBlock,
    EthLocalBlockForChainSpec, RemoteBlock, SyncBlock,
};
use edr_provider::{time::TimeSinceEpoch, ProviderSpec, TransactionFailureReason};

use crate::GenericChainSpec;

impl ChainHardfork for GenericChainSpec {
    type Hardfork = l1::SpecId;
}

impl ChainSpec for GenericChainSpec {
    type BlockEnv = l1::BlockEnv;
    type Context = ();
    type HaltReason = l1::HaltReason;
    type SignedTransaction = crate::transaction::SignedWithFallbackToPostEip155;
    type BlockConstructor = GenericBlockConstructor;
}

/// Generic chain definition for `BlockEnvConstructor`
pub struct GenericBlockConstructor;

impl BlockEnvConstructor<Header, BlockEnv> for GenericBlockConstructor {
    fn new_block_env(header: &Header, hardfork: l1::SpecId) -> BlockEnv {
        BlockEnv {
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
            blob_excess_gas_and_price: header
                .blob_gas
                .as_ref()
                .map(|BlobGas { excess_gas, .. }| {
                    eip4844::BlobExcessGasAndPrice::new(*excess_gas, hardfork >= l1::SpecId::PRAGUE)
                })
                .or_else(|| Some(eip4844::BlobExcessGasAndPrice::new(0u64, false))),
        }
    }
}

impl BlockEnvConstructor<PartialHeader, BlockEnv> for GenericBlockConstructor {
    fn new_block_env(header: &PartialHeader, hardfork: l1::SpecId) -> BlockEnv {
        BlockEnv {
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
            blob_excess_gas_and_price: header
                .blob_gas
                .as_ref()
                .map(|BlobGas { excess_gas, .. }| {
                    eip4844::BlobExcessGasAndPrice::new(*excess_gas, hardfork >= l1::SpecId::PRAGUE)
                })
                .or_else(|| Some(eip4844::BlobExcessGasAndPrice::new(0u64, false))),
        }
    }
}

impl EthHeaderConstants for GenericChainSpec {
    const BASE_FEE_PARAMS: BaseFeeParams<Self::Hardfork> = L1ChainSpec::BASE_FEE_PARAMS;

    const MIN_ETHASH_DIFFICULTY: u64 = L1ChainSpec::MIN_ETHASH_DIFFICULTY;
}

impl GenesisBlockFactory for GenericChainSpec {
    type CreationError = <L1ChainSpec as GenesisBlockFactory>::CreationError;

    type LocalBlock = <Self as RuntimeSpec>::LocalBlock;

    fn genesis_block(
        genesis_diff: edr_evm::state::StateDiff,
        hardfork: Self::Hardfork,
        mut options: edr_evm::GenesisBlockOptions,
    ) -> Result<Self::LocalBlock, Self::CreationError> {
        // If no option is provided, use the default extra data for L1 Ethereum.
        options.extra_data = Some(
            options
                .extra_data
                .unwrap_or(Bytes::copy_from_slice(EXTRA_DATA)),
        );

        EthLocalBlockForChainSpec::<Self>::with_genesis_state::<Self>(
            genesis_diff,
            hardfork,
            options,
        )
    }
}

impl RuntimeSpec for GenericChainSpec {
    type Block = dyn SyncBlock<
        Arc<Self::BlockReceipt>,
        Self::SignedTransaction,
        Error = <Self::LocalBlock as BlockReceipts<Arc<Self::BlockReceipt>>>::Error,
    >;

    type BlockBuilder<
        'builder,
        BlockchainErrorT: 'builder + std::error::Error + Send,
        StateErrorT: 'builder + std::error::Error + Send,
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

    type ReceiptBuilder = crate::receipt::execution::Builder;
    type RpcBlockConversionError = crate::rpc::block::ConversionError<Self>;
    type RpcReceiptConversionError = crate::rpc::receipt::ConversionError;
    type RpcTransactionConversionError = crate::rpc::transaction::ConversionError;

    fn cast_local_block(local_block: Arc<Self::LocalBlock>) -> Arc<Self::Block> {
        local_block
    }

    fn cast_remote_block(remote_block: Arc<RemoteBlock<Self>>) -> Arc<Self::Block> {
        remote_block
    }

    fn cast_transaction_error<BlockchainErrorT, StateErrorT>(
        error: <Self::SignedTransaction as TransactionValidation>::ValidationError,
    ) -> TransactionErrorForChainSpec<BlockchainErrorT, Self, StateErrorT> {
        // Can't use L1ChainSpec impl here as the TransactionError is generic
        // over the specific chain spec rather than just the validation error.
        // Instead, we copy the impl here.
        match error {
            InvalidTransaction::LackOfFundForMaxFee { fee, balance } => {
                TransactionError::LackOfFundForMaxFee { fee, balance }
            }
            remainder => TransactionError::InvalidTransaction(remainder),
        }
    }

    fn chain_hardfork_activations(chain_id: u64) -> Option<&'static Activations<Self::Hardfork>> {
        L1ChainSpec::chain_hardfork_activations(chain_id)
    }

    fn chain_name(chain_id: u64) -> Option<&'static str> {
        L1ChainSpec::chain_name(chain_id)
    }

    fn evm<
        BlockchainErrorT,
        DatabaseT: Database<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        PrecompileProviderT: PrecompileProvider<ContextForChainSpec<Self, DatabaseT>, Output = InterpreterResult>,
        StateErrorT,
    >(
        context: ContextForChainSpec<Self, DatabaseT>,
        precompile_provider: PrecompileProviderT,
    ) -> Self::Evm<BlockchainErrorT, DatabaseT, NoOpInspector, PrecompileProviderT, StateErrorT>
    {
        Self::evm_with_inspector(context, NoOpInspector {}, precompile_provider)
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

impl<TimerT: Clone + TimeSinceEpoch> ProviderSpec<TimerT> for GenericChainSpec {
    type PooledTransaction = edr_eth::transaction::pooled::PooledTransaction;
    type TransactionRequest = crate::transaction::Request;

    fn cast_halt_reason(reason: Self::HaltReason) -> TransactionFailureReason<Self::HaltReason> {
        <L1ChainSpec as ProviderSpec<TimerT>>::cast_halt_reason(reason)
    }
}
