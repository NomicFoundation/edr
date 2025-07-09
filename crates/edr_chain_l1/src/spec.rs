use alloy_rlp::RlpEncodable;
use edr_eth::{
    eips::eip1559::{BaseFeeParams, ConstantBaseFeeParams},
    spec::{ChainSpec, EthHeaderConstants},
};
use edr_rpc_eth::RpcSpec;

use crate::{eip2718::TypedEnvelope, rpc, transaction, L1BlockEnv, L1HaltReason, L1Hardfork};

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

impl RpcSpec for L1ChainSpec {
    type ExecutionReceipt<LogT> = TypedEnvelope<edr_eth::receipt::execution::Eip658<LogT>>;
    type RpcBlock<DataT>
        = rpc::Block<DataT>
    where
        DataT: Default + DeserializeOwned + Serialize;
    type RpcCallRequest = CallRequest;
    type RpcReceipt = Block;
    type RpcTransaction = rpc::transaction::TransactionWithSignature;
    type RpcTransactionRequest = rpc::transaction::Request;
}
