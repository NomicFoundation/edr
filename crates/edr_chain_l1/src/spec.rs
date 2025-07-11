use std::sync::Arc;

use alloy_rlp::RlpEncodable;
use edr_eth::{
    eips::eip1559::{BaseFeeParams, ConstantBaseFeeParams},
    log::FilterLog,
    receipt::BlockReceipt,
    spec::{ChainSpec, EthHeaderConstants},
    transaction::TransactionValidation,
};
use edr_evm::{
    evm::{Evm, EvmData},
    hardfork::Activations,
    inspector::Inspector,
    interpreter::{EthInstructions, EthInterpreter, InterpreterResult},
    precompile::EthPrecompiles,
    spec::{ContextForChainSpec, ExecutionReceiptTypeConstructorForChainSpec, RuntimeSpec},
    state::{Database, DatabaseComponentError},
    transaction::{TransactionError, TransactionErrorForChainSpec},
    BlockReceipts, EthBlockBuilder, EthBlockReceiptFactory, EthLocalBlock, EvmInvalidTransaction,
    RemoteBlock, RemoteBlockConversionError, SyncBlock,
};
use edr_provider::ProviderSpec;
use edr_rpc_eth::{CallRequest, RpcSpec};
use revm_handler::PrecompileProvider;
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    eip2718::TypedEnvelope,
    hardfork::{chain_hardfork_activations, chain_name},
    receipt::L1ReceiptBuilder,
    rpc, transaction, L1BlockEnv, L1HaltReason, L1Hardfork,
};

/// The chain specification for Ethereum Layer 1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, RlpEncodable)]
pub struct L1ChainSpec;

impl ChainSpec for L1ChainSpec {
    type BlockEnv = L1BlockEnv;
    type Context = ();
    type HaltReason = L1HaltReason;
    type Hardfork = L1Hardfork;
    type SignedTransaction = transaction::Signed;
}

impl EthHeaderConstants for L1ChainSpec {
    const BASE_FEE_PARAMS: BaseFeeParams<Self::Hardfork> =
        BaseFeeParams::Constant(ConstantBaseFeeParams::ethereum());

    const MIN_ETHASH_DIFFICULTY: u64 = 131072;
}

impl<TimerT: Clone + TimeSinceEpoch> ProviderSpec<TimerT> for L1ChainSpec {
    type PooledTransaction = transaction::pooled::PooledTransaction;
    type TransactionRequest = transaction::Request;

    fn cast_halt_reason(reason: Self::HaltReason) -> TransactionFailureReason<Self::HaltReason> {
        match reason {
            Self::HaltReason::CreateContractSizeLimit => {
                TransactionFailureReason::CreateContractSizeLimit
            }
            Self::HaltReason::OpcodeNotFound | Self::HaltReason::InvalidFEOpcode => {
                TransactionFailureReason::OpcodeNotFound
            }
            Self::HaltReason::OutOfGas(error) => TransactionFailureReason::OutOfGas(error),
            remainder => TransactionFailureReason::Inner(remainder),
        }
    }
}

impl RpcSpec for L1ChainSpec {
    type ExecutionReceipt<LogT> = TypedEnvelope<edr_eth::receipt::execution::Eip658<LogT>>;
    type RpcBlock<DataT>
        = rpc::Block<DataT>
    where
        DataT: Default + DeserializeOwned + Serialize;
    type RpcCallRequest = CallRequest;
    type RpcReceipt = rpc::receipt::Block;
    type RpcTransaction = rpc::transaction::TransactionWithSignature;
    type RpcTransactionRequest = rpc::transaction::Request;
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

    type ReceiptBuilder = L1ReceiptBuilder;
    type RpcBlockConversionError = RemoteBlockConversionError<Self::RpcTransactionConversionError>;
    type RpcReceiptConversionError = rpc::receipt::ConversionError;
    type RpcTransactionConversionError = rpc::transaction::ConversionError;

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
            EvmInvalidTransaction::LackOfFundForMaxFee { fee, balance } => {
                TransactionError::LackOfFundForMaxFee { fee, balance }
            }
            remainder => TransactionError::InvalidTransaction(remainder),
        }
    }

    fn chain_hardfork_activations(chain_id: u64) -> Option<&'static Activations<Self::Hardfork>> {
        chain_hardfork_activations(chain_id)
    }

    fn chain_name(chain_id: u64) -> Option<&'static str> {
        chain_name(chain_id)
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
            data: EvmData {
                ctx: context,
                inspector,
            },
            instruction: EthInstructions::default(),
            precompiles: precompile_provider,
        }
    }
}
