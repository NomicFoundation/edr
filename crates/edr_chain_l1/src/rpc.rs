/// Types for Ethereum L1 JSON-RPC blocks.
pub mod block;
/// Types for Ethereum L1 JSON-RPC receipts.
pub mod receipt;
/// Types for Ethereum L1 JSON-RPC transactions.
pub mod transaction;

pub use self::receipt::L1RpcBlockReceipt as Block;
