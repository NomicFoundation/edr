mod call_request;
/// Types for Ethereum L1 JSON-RPC receipts.
pub mod receipt;
/// Types for Ethereum L1 JSON-RPC transactions.
pub mod transaction;

pub use self::{block::L1RpcBlock as Block, receipt::L1RpcBlockReceipt as BlockReceipt};
