use serde::{de::DeserializeOwned, Serialize};

/// Trait for specifying chain-specific RPC client types.
pub trait ChainSpec {
    /// Type representing a block
    type Block<Data>: GetBlockNumber + DeserializeOwned + Serialize
    where
        Data: Default + DeserializeOwned + Serialize;

    /// Type representing the transaction.
    type Transaction: Default + DeserializeOwned + Serialize;
}

pub trait GetBlockNumber {
    fn number(&self) -> Option<u64>;
}

/// Chain specification for the Ethereum JSON-RPC API.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EthChainSpec;

impl ChainSpec for EthChainSpec {
    type Block<Data> = crate::block::Block<Data> where Data: Default + DeserializeOwned + Serialize;
    type Transaction = crate::transaction::Transaction;
}
