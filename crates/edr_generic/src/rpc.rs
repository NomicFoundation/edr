use edr_chain_l1::rpc::{call::L1CallRequest, TransactionRequest};
use edr_rpc_spec::RpcChainSpec;
use serde::{de::DeserializeOwned, Serialize};

use crate::{eip2718::TypedEnvelope, GenericChainSpec};

pub mod block;
pub mod receipt;
pub mod transaction;

impl RpcChainSpec for GenericChainSpec {
    type ExecutionReceipt<Log> = TypedEnvelope<edr_receipt::execution::Eip658<Log>>;
    type RpcBlock<Data>
        = self::block::GenericRpcBlock<Data>
    where
        Data: Default + DeserializeOwned + Serialize;
    type RpcCallRequest = L1CallRequest;
    type RpcReceipt = self::receipt::BlockReceipt;
    type RpcTransaction = self::transaction::TransactionWithSignature;
    type RpcTransactionRequest = TransactionRequest;
}
