use edr_eth::{chain_spec::L1ChainSpec, receipt::Receipt};
use serde::{de::DeserializeOwned, Serialize};

/// Trait for specifying Ethereum-based JSON-RPC method types.
pub trait RpcSpec: Sized {
    /// Type representing an RPC execution receipt.
    type ExecutionReceipt<Log>: Receipt<Log> + DeserializeOwned + Serialize
    where
        Log: DeserializeOwned + Serialize;

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

impl RpcSpec for L1ChainSpec {
    type ExecutionReceipt<Log> = edr_eth::receipt::Execution<Log> where Log: DeserializeOwned + Serialize;
    type RpcBlock<Data> = crate::block::Block<Data> where Data: Default + DeserializeOwned + Serialize;
    type RpcTransaction = crate::transaction::Transaction;
}

pub type BlockReceipt<RpcSpecT> = edr_eth::receipt::BlockReceipt<
    <RpcSpecT as RpcSpec>::ExecutionReceipt<edr_eth::log::FilterLog>,
>;
