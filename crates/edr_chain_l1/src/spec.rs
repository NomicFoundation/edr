use alloy_rlp::RlpEncodable;
use edr_eip1559::{BaseFeeParams, ConstantBaseFeeParams};
use edr_evm_spec::{ChainHardfork, ChainSpec, EthHeaderConstants};
use edr_rpc_spec::RpcSpec;
use serde::{de::DeserializeOwned, Serialize};

use crate::{rpc, BlockEnv, HaltReason, Hardfork, Signed, TypedEnvelope};

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
    type SignedTransaction = Signed;
}

impl EthHeaderConstants for L1ChainSpec {
    fn base_fee_params() -> BaseFeeParams<Self::Hardfork> {
        BaseFeeParams::Constant(ConstantBaseFeeParams::ethereum())
    }

    const MIN_ETHASH_DIFFICULTY: u64 = 131072;
}

impl RpcSpec for L1ChainSpec {
    type ExecutionReceipt<LogT> = TypedEnvelope<edr_receipt::execution::Eip658<LogT>>;
    type RpcBlock<DataT>
        = rpc::Block<DataT>
    where
        DataT: Default + DeserializeOwned + Serialize;
    type RpcCallRequest = rpc::CallRequest;
    type RpcReceipt = rpc::Block;
    type RpcTransaction = rpc::TransactionWithSignature;
    type RpcTransactionRequest = rpc::TransactionRequest;
}
