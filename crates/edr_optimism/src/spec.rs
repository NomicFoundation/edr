use core::fmt::Debug;
use std::{marker::PhantomData, sync::Arc};

use alloy_rlp::RlpEncodable;
use edr_eth::{
    eips::eip1559::{BaseFeeParams, ConstantBaseFeeParams, ForkBaseFeeParams},
    l1,
    spec::{ChainSpec, EthHeaderConstants},
};
use edr_evm::{
    inspector::NoOpInspector,
    spec::{ContextForChainSpec, RuntimeSpec},
    state::Database,
    transaction::{TransactionError, TransactionErrorForChainSpec, TransactionValidation},
    BlockReceipts, RemoteBlock, RemoteBlockConversionError, SyncBlock,
};
use edr_napi_core::{
    napi,
    spec::{marshal_response_data, Response, SyncNapiSpec},
};
use edr_provider::{time::TimeSinceEpoch, ProviderSpec, TransactionFailureReason};
use edr_rpc_eth::{jsonrpc, spec::RpcSpec};
use op_revm::{L1BlockInfo, OpBuilder as _, OpEvm};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    block::{self, LocalBlock},
    eip2718::TypedEnvelope,
    hardfork,
    receipt::{self, BlockReceiptFactory},
    rpc, transaction,
    transaction::InvalidTransaction,
    OpHaltReason, OpSpecId,
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

/// EVM wiring for Optimism chains.
pub struct Wiring<DatabaseT: Database, ExternalContextT> {
    _phantom: PhantomData<(DatabaseT, ExternalContextT)>,
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
        StateErrorT,
    > = OpEvm<ContextForChainSpec<Self, DatabaseT>, InspectorT>;

    type LocalBlock = LocalBlock;

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

    fn evm<
        BlockchainErrorT,
        DatabaseT: Database<Error = edr_evm::state::DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        StateErrorT,
    >(
        context: ContextForChainSpec<Self, DatabaseT>,
    ) -> Self::Evm<BlockchainErrorT, DatabaseT, edr_evm::inspector::NoOpInspector, StateErrorT>
    {
        context.build_op_with_inspector(NoOpInspector {})
    }

    fn evm_with_inspector<
        BlockchainErrorT,
        DatabaseT: Database<Error = edr_evm::state::DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        InspectorT: edr_evm::inspector::Inspector<ContextForChainSpec<Self, DatabaseT>>,
        StateErrorT,
    >(
        context: ContextForChainSpec<Self, DatabaseT>,
        inspector: InspectorT,
    ) -> Self::Evm<BlockchainErrorT, DatabaseT, InspectorT, StateErrorT> {
        context.build_op_with_inspector(inspector)
    }
}

impl EthHeaderConstants for OpChainSpec {
    const BASE_FEE_PARAMS: BaseFeeParams<OpSpecId> =
        BaseFeeParams::Variable(ForkBaseFeeParams::new(&[
            (OpSpecId::BEDROCK, ConstantBaseFeeParams::new(50, 6)),
            (OpSpecId::CANYON, ConstantBaseFeeParams::new(250, 6)),
        ]));

    const MIN_ETHASH_DIFFICULTY: u64 = 0;
}

impl SyncNapiSpec for OpChainSpec {
    const CHAIN_TYPE: &'static str = "Optimism";

    fn cast_response(
        response: Result<
            edr_provider::ResponseWithTraces<OpHaltReason>,
            edr_provider::ProviderErrorForChainSpec<Self>,
        >,
    ) -> napi::Result<edr_napi_core::spec::Response<l1::HaltReason>> {
        let response = jsonrpc::ResponseData::from(response.map(|response| response.result));

        marshal_response_data(response).map(|data| Response {
            solidity_trace: None,
            data,
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
