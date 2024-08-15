use edr_eth::{chain_spec::L1ChainSpec, eips::eip2718::TypedEnvelope, receipt::Receipt};
use serde::{de::DeserializeOwned, Serialize};

use crate::{receipt::Block, CallRequest};

/// Trait for specifying Ethereum-based JSON-RPC method types.
pub trait RpcSpec: Sized {
    /// Type representing an RPC execution receipt.
    type ExecutionReceipt<Log>: Receipt<Log>;

    /// Type representing an RPC block
    type RpcBlock<Data>: GetBlockNumber + DeserializeOwned + Serialize
    where
        Data: Default + DeserializeOwned + Serialize;

    /// Type representing an RPC `eth_call` request.
    type RpcCallRequest: DeserializeOwned + Serialize;

    /// Type representing an RPC receipt.
    type RpcReceipt: DeserializeOwned + Serialize;

    /// Type representing an RPC transaction.
    type RpcTransaction: Default + DeserializeOwned + Serialize;

    /// Type representing an RPC `eth_sendTransaction` request.
    type RpcTransactionRequest: DeserializeOwned;
}

pub trait GetBlockNumber {
    fn number(&self) -> Option<u64>;
}

impl RpcSpec for L1ChainSpec {
    type ExecutionReceipt<Log> = TypedEnvelope<edr_eth::receipt::Execution<Log>>;
    type RpcBlock<Data> = crate::block::Block<Data> where Data: Default + DeserializeOwned + Serialize;
    type RpcCallRequest = CallRequest;
    type RpcReceipt = Block;
    type RpcTransaction = crate::transaction::TransactionWithSignature;
    type RpcTransactionRequest = crate::transaction::TransactionRequest;
}
