mod call_request;
/// Types for Ethereum L1 JSON-RPC receipts.
pub mod receipt;
/// Types for Ethereum L1 JSON-RPC transactions.
pub mod transaction;

/// Convenience type alias for [`L1RpcBlockReceipt`].
///
/// This allows usage like `edr_chain_l1::rpc::BlockReceipt`.
pub type BlockReceipt = self::receipt::L1RpcBlockReceipt;

/// Convenience type alias for [`L1RpcTransaction`].
///
/// This allows usage like `edr_chain_l1::rpc::Transaction`.
pub type Transaction = self::transaction::L1RpcTransaction;
