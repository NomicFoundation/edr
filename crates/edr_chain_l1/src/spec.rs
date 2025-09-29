use alloy_rlp::RlpEncodable;
use edr_evm_spec::{ChainHardfork, ChainSpec, EthHeaderConstants};
use edr_rpc_eth::ChainRpcBlock;
use edr_rpc_spec::RpcSpec;
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    rpc::{
        block::L1RpcBlock,
        call::L1CallRequest,
        receipt::L1BlockReceipt,
        transaction::{L1RpcTransactionRequest, L1RpcTransactionWithSignature},
    },
    BlockEnv, HaltReason, Hardfork, L1SignedTransaction, TypedEnvelope,
};

/// The chain specification for Ethereum Layer 1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, RlpEncodable)]
pub struct L1ChainSpec;

impl ChainHardfork for L1ChainSpec {
    type Hardfork = Hardfork;
}

impl ChainSpec for L1ChainSpec {
    type BlockEnv = BlockEnv;
    type Context = ();
    type HaltReason = HaltReason;
    type SignedTransaction = L1SignedTransaction;
}

impl EthHeaderConstants for L1ChainSpec {
    const MIN_ETHASH_DIFFICULTY: u64 = 131072;
}

impl ChainRpcBlock for L1ChainSpec {
    type RpcBlock<DataT>
        = L1RpcBlock<DataT>
    where
        DataT: Default + DeserializeOwned + Serialize;
}

impl RpcSpec for L1ChainSpec {
    type ExecutionReceipt<LogT> = TypedEnvelope<edr_receipt::execution::Eip658<LogT>>;
    type RpcCallRequest = L1CallRequest;
    type RpcReceipt = L1BlockReceipt;
    type RpcTransaction = L1RpcTransactionWithSignature;
    type RpcTransactionRequest = L1RpcTransactionRequest;
}
