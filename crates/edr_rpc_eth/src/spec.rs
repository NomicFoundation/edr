use edr_eth::{chain_spec::L1ChainSpec, eips::eip2718::TypedEnvelope, receipt::Receipt};
use serde::{de::DeserializeOwned, Serialize};

use crate::receipt::Block;

/// Trait for specifying Ethereum-based JSON-RPC method types.
pub trait RpcSpec: Sized {
    /// Type representing an RPC execution receipt.
    type ExecutionReceipt<Log>: Receipt<Log>;

    /// Type representing an RPC block
    type RpcBlock<Data>: GetBlockNumber + DeserializeOwned + Serialize
    where
        Data: Default + DeserializeOwned + Serialize;

    /// Type representing an RPC receipt.
    type RpcReceipt: DeserializeOwned + Serialize;

    /// Type representing an RPC transaction.
    type RpcTransaction: Default + DeserializeOwned + Serialize;
}

pub trait GetBlockNumber {
    fn number(&self) -> Option<u64>;
}

impl RpcSpec for L1ChainSpec {
    type ExecutionReceipt<Log> = TypedEnvelope<edr_eth::receipt::Execution<Log>>;
    type RpcBlock<Data> = crate::block::Block<Data> where Data: Default + DeserializeOwned + Serialize;
    type RpcReceipt = Block;
    type RpcTransaction = crate::transaction::Transaction;
}
