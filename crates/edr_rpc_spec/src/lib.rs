#![warn(missing_docs)]
//! Ethereum JSON-RPC specification types

use edr_receipt::ExecutionReceipt;
use serde::{de::DeserializeOwned, Serialize};

/// Trait for specifying Ethereum-based JSON-RPC method types.
pub trait RpcSpec {
    /// Type representing an RPC execution receipt.
    type ExecutionReceipt<LogT>: ExecutionReceipt<Log = LogT>;

    /// Type representing an RPC block
    type RpcBlock<DataT>: GetBlockNumber + DeserializeOwned + Serialize
    where
        DataT: Default + DeserializeOwned + Serialize;

    /// Type representing an RPC `eth_call` request.
    type RpcCallRequest: DeserializeOwned + Serialize;

    /// Type representing an RPC receipt.
    type RpcReceipt: DeserializeOwned + Serialize;

    /// Type representing an RPC transaction.
    type RpcTransaction: Default + DeserializeOwned + Serialize;

    /// Type representing an RPC `eth_sendTransaction` request.
    type RpcTransactionRequest: DeserializeOwned + Serialize;
}

/// Trait for retrieving a block's number.
pub trait GetBlockNumber {
    fn number(&self) -> Option<u64>;
}

/// Trait for constructing an RPC type from an internal type.
pub trait RpcTypeFrom<InputT> {
    /// The hardfork type.
    type Hardfork;

    /// Constructs an RPC type from the provided internal value.
    fn rpc_type_from(value: &InputT, hardfork: Self::Hardfork) -> Self;
}
