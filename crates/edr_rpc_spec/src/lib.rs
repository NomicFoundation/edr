#![warn(missing_docs)]
//! Ethereum JSON-RPC specification types

#[cfg(any(feature = "test-utils", test))]
mod test_utils;

use edr_primitives::{B256, U256};
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

/// Trait that provides access to common properties of an Ethereum-based RPC
/// block.
pub trait RpcEthBlock {
    /// Returns the root of the block's state trie.
    fn state_root(&self) -> &B256;

    /// Returns the block's timestamp.
    fn timestamp(&self) -> u64;

    /// Returns the total difficulty of the chain until this block for finalized
    /// blocks. For pending blocks, returns `None`.
    fn total_difficulty(&self) -> Option<&U256>;
}

/// Trait for retrieving a block's number.
pub trait GetBlockNumber {
    /// Retrieves the block number, if available. If the block is pending,
    /// returns `None`.
    fn number(&self) -> Option<u64>;
}

/// Trait for constructing an RPC type from an internal type.
pub trait RpcTypeFrom<InputT> {
    /// The hardfork type.
    type Hardfork;

    /// Constructs an RPC type from the provided internal value.
    fn rpc_type_from(value: &InputT, hardfork: Self::Hardfork) -> Self;
}
