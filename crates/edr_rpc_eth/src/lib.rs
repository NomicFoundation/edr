mod block;
mod cacheable_method_invocation;
/// Input type for `eth_call` and `debug_traceCall`.
mod call_request;
/// Types related to the Ethereum JSON-RPC API.
pub mod client;
/// Types related to forking a remote blockchain.
pub mod fork;
mod r#override;
mod request_methods;
/// Types for Ethereum JSON-RPC API specification.
pub mod spec;
#[cfg(any(feature = "test-utils", test))]
mod test_utils;
/// Input type for `eth_sendTransaction`.
mod transaction_request;

pub use edr_rpc_client::{error, header, jsonrpc, HeaderMap};

pub use self::{
    block::RpcBlock, call_request::CallRequest, r#override::*, request_methods::RequestMethod,
    spec::RpcSpec, transaction_request::RpcTransactionRequest,
};

/// Trait for constructing an RPC type from an internal type.
pub trait RpcTypeFrom<InputT> {
    /// The hardfork type.
    type Hardfork;

    /// Constructs an RPC type from the provided internal value.
    fn rpc_type_from(value: &InputT, hardfork: Self::Hardfork) -> Self;
}
