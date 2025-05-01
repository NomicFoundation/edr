use core::fmt::Debug;
use std::sync::Arc;

use alloy_rlp::RlpEncodable;
use edr_eth::{
    eips::eip1559::{BaseFeeParams, ConstantBaseFeeParams, ForkBaseFeeParams},
    l1,
    spec::{ChainSpec, EthHeaderParams},
};
use edr_evm::{
    BlockReceipts, RemoteBlock, RemoteBlockConversionError, SyncBlock,
    evm::{Evm, EvmData},
    interpreter::{EthInstructions, EthInterpreter, InterpreterResult},
    precompile::PrecompileProvider,
    spec::{ContextForChainSpec, RuntimeSpec},
    state::Database,
    transaction::{TransactionError, TransactionErrorForChainSpec, TransactionValidation},
};
use edr_napi_core::{
    napi,
    spec::{Response, SyncNapiSpec, marshal_response_data},
};
use edr_provider::{ProviderSpec, TransactionFailureReason, time::TimeSinceEpoch};
use edr_rpc_eth::{jsonrpc, spec::RpcSpec};
use edr_solidity::contract_decoder::ContractDecoder;
use op_revm::{L1BlockInfo, OpEvm, precompiles::OpPrecompiles};
use serde::{Serialize, de::DeserializeOwned};

use crate::{
    OpHaltReason, OpSpecId,
    block::{self, LocalBlock},
    eip2718::TypedEnvelope,
    hardfork,
    receipt::{self, BlockReceiptFactory},
    rpc,
    transaction::{self, InvalidTransaction},
};

/// Chain specification for the Ethereum JSON-RPC API.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, RlpEncodable)]
pub struct OpChainSpec;

impl RpcSpec for OpChainSpec {
    type ExecutionReceipt<Log> = TypedEnvelope<receipt::Execution<Log>>;
    type RpcBlock<Data>
        = edr_rpc_eth::Block<Data>
    where
        Data: Default + DeserializeOwned + Serialize;
    type RpcCallRequest = edr_rpc_eth::CallRequest;
    type RpcReceipt = rpc::BlockReceipt;
    type RpcTransaction = rpc::Transaction;
    type RpcTransactionRequest = edr_rpc_eth::TransactionRequest;
}

impl ChainSpec for OpChainSpec {
    type BlockEnv = l1::BlockEnv;
    type Context = L1BlockInfo;
    type HaltReason = OpHaltReason;
    type Hardfork = OpSpecId;
    type SignedTransaction = transaction::Signed;
}

impl RuntimeSpec for OpChainSpec {
    type Block = dyn SyncBlock<
            Arc<Self::BlockReceipt>,
            Self::SignedTransaction,
            Error = <Self::LocalBlock as BlockReceipts<Arc<Self::BlockReceipt>>>::Error,
        >;

    type BlockBuilder<
        'blockchain,
        BlockchainErrorT: 'blockchain + Send + std::error::Error,
        StateErrorT: 'blockchain + Send + std::error::Error,
    > = block::Builder<'blockchain, BlockchainErrorT, StateErrorT>;

    type BlockReceipt = receipt::Block;
    type BlockReceiptFactory = BlockReceiptFactory;

    type Evm<
        BlockchainErrorT,
        DatabaseT: Database<Error = edr_evm::state::DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        InspectorT: edr_evm::inspector::Inspector<edr_evm::spec::ContextForChainSpec<Self, DatabaseT>>,
        PrecompileProviderT: PrecompileProvider<ContextForChainSpec<Self, DatabaseT>, Output = InterpreterResult>,
        StateErrorT,
    > = OpEvm<
        ContextForChainSpec<Self, DatabaseT>,
        InspectorT,
        EthInstructions<EthInterpreter, ContextForChainSpec<Self, DatabaseT>>,
        PrecompileProviderT,
    >;

    type LocalBlock = LocalBlock;

    type PrecompileProvider<
        BlockchainErrorT,
        DatabaseT: Database<Error = edr_evm::state::DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        StateErrorT,
    > = OpPrecompiles;

    type ReceiptBuilder = receipt::execution::Builder;
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
            InvalidTransaction::Base(l1::InvalidTransaction::LackOfFundForMaxFee {
                fee,
                balance,
            }) => TransactionError::LackOfFundForMaxFee { fee, balance },
            remainder => TransactionError::InvalidTransaction(remainder),
        }
    }

    fn chain_hardfork_activations(
        chain_id: u64,
    ) -> Option<&'static edr_evm::hardfork::Activations<Self::Hardfork>> {
        hardfork::chain_hardfork_activations(chain_id)
    }

    fn chain_name(chain_id: u64) -> Option<&'static str> {
        hardfork::chain_name(chain_id)
    }

    fn evm_with_inspector<
        BlockchainErrorT,
        DatabaseT: Database<Error = edr_evm::state::DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        InspectorT: edr_evm::inspector::Inspector<ContextForChainSpec<Self, DatabaseT>>,
        PrecompileProviderT: PrecompileProvider<ContextForChainSpec<Self, DatabaseT>, Output = InterpreterResult>,
        StateErrorT,
    >(
        context: ContextForChainSpec<Self, DatabaseT>,
        inspector: InspectorT,
        precompile_provider: PrecompileProviderT,
    ) -> Self::Evm<BlockchainErrorT, DatabaseT, InspectorT, PrecompileProviderT, StateErrorT> {
        OpEvm(Evm {
            data: EvmData {
                ctx: context,
                inspector,
            },
            instruction: EthInstructions::new_mainnet(),
            precompiles: precompile_provider,
        })
    }
}

impl EthHeaderParams for OpChainSpec {
    const MIN_ETHASH_DIFFICULTY: u64 = 0;

    fn chain_base_fee_params(chain_id: u64) -> BaseFeeParams<Self::Hardfork> {
        const DEFAULT_BASE_FEE_PARAMS: BaseFeeParams<OpSpecId> =
            BaseFeeParams::Variable(ForkBaseFeeParams::new(&[
                (OpSpecId::BEDROCK, ConstantBaseFeeParams::new(50, 6)),
                (OpSpecId::CANYON, ConstantBaseFeeParams::new(250, 6)),
            ]));

        hardfork::chain_base_fee_params(chain_id).copied().unwrap_or(DEFAULT_BASE_FEE_PARAMS)
    }
}

impl SyncNapiSpec for OpChainSpec {
    const CHAIN_TYPE: &'static str = crate::CHAIN_TYPE;

    fn cast_response(
        response: Result<
            edr_provider::ResponseWithTraces<OpHaltReason>,
            edr_provider::ProviderErrorForChainSpec<Self>,
        >,
        _contract_decoder: Arc<ContractDecoder>,
    ) -> napi::Result<edr_napi_core::spec::Response<l1::HaltReason>> {
        let response = jsonrpc::ResponseData::from(response.map(|response| response.result));

        marshal_response_data(response).map(|data| Response {
            data,
            // TODO: Add support for Solidity stack traces in OP
            solidity_trace: None,
            traces: Vec::new(),
        })
    }
}

impl<TimerT: Clone + TimeSinceEpoch> ProviderSpec<TimerT> for OpChainSpec {
    type PooledTransaction = transaction::Pooled;
    type TransactionRequest = transaction::Request;

    fn cast_halt_reason(reason: OpHaltReason) -> TransactionFailureReason<OpHaltReason> {
        match reason {
            OpHaltReason::Base(reason) => match reason {
                l1::HaltReason::CreateContractSizeLimit => {
                    TransactionFailureReason::CreateContractSizeLimit
                }
                l1::HaltReason::OpcodeNotFound | l1::HaltReason::InvalidFEOpcode => {
                    TransactionFailureReason::OpcodeNotFound
                }
                l1::HaltReason::OutOfGas(error) => TransactionFailureReason::OutOfGas(error),
                remainder => TransactionFailureReason::Inner(OpHaltReason::Base(remainder)),
            },
            remainder => TransactionFailureReason::Inner(remainder),
        }
    }
}
