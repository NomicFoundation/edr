mod cacheable_method_invocation;
/// Types related to the Ethereum JSON-RPC API.
pub mod client;
/// Types related to forking a remote blockchain.
pub mod fork;
mod r#override;
mod request_methods;

pub use edr_rpc_client::{error, header, jsonrpc, HeaderMap};
use serde::{de::DeserializeOwned, Serialize};

pub use self::{r#override::*, request_methods::RequestMethod};

/// Trait for retrieving a block's number.
pub trait GetBlockNumber {
    /// Retrieves the block number, if available. If the block is pending,
    /// returns `None`.
    fn number(&self) -> Option<u64>;
}

/// Trait for specifying Ethereum-based JSON-RPC block types for a chain
/// type.
pub trait ChainRpcBlock {
    /// Type representing an RPC block
    type RpcBlock<DataT>: GetBlockNumber + DeserializeOwned + Serialize
    where
        DataT: Default + DeserializeOwned + Serialize;
}
