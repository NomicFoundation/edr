/// Types for Ethereum JSON-RPC blocks
mod block;
mod cacheable_method_invocation;
/// Input type for `eth_call` and `eth_estimateGas`
mod call_request;
/// Types related to the Ethereum JSON-RPC API
pub mod client;
/// Types related to forking a remote blockchain.
pub mod fork;
mod r#override;
mod request_methods;
/// Types for Ethereum JSON-RPC API specification.
pub mod spec;
mod transaction;

pub use edr_rpc_client::{error, header, jsonrpc, HeaderMap};

pub use self::{
    block::Block,
    call_request::CallRequest,
    r#override::*,
    request_methods::RequestMethod,
    transaction::{ConversionError as TransactionConversionError, Transaction},
};
