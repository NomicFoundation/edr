use edr_rpc_eth::RpcSpec;
use serde::{Serialize, de::DeserializeOwned};

use crate::{GenericChainSpec, eip2718::TypedEnvelope};

pub mod block;
pub mod receipt;
pub mod transaction;

impl RpcSpec for GenericChainSpec {
    type ExecutionReceipt<Log> = TypedEnvelope<edr_eth::receipt::execution::Eip658<Log>>;
    type RpcBlock<Data>
        = self::block::Block<Data>
    where
        Data: Default + DeserializeOwned + Serialize;
    type RpcCallRequest = edr_rpc_eth::CallRequest;
    type RpcReceipt = self::receipt::BlockReceipt;
    type RpcTransaction = self::transaction::TransactionWithSignature;
    type RpcTransactionRequest = edr_rpc_eth::TransactionRequest;
}
