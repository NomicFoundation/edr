use serde::{de::DeserializeOwned, Serialize};

/// Trait for specifying Ethereum-based JSON-RPC method types.
pub trait RpcSpec {
    /// Type representing an RPC block
    type RpcBlock<Data>: GetBlockNumber + DeserializeOwned + Serialize
    where
        Data: Default + DeserializeOwned + Serialize;

    /// Type representing an RPC transaction.
    type RpcTransaction: Default + DeserializeOwned + Serialize;
}

pub trait GetBlockNumber {
    fn number(&self) -> Option<u64>;
}

/// Chain specification for the Ethereum JSON-RPC API.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EthRpcSpec;

impl RpcSpec for EthRpcSpec {
    type RpcBlock<Data> = crate::block::Block<Data> where Data: Default + DeserializeOwned + Serialize;
    type RpcTransaction = crate::transaction::Transaction;
}
