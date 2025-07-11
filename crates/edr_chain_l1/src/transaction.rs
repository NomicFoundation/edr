/// Types for transaction gossip (aka pooled transactions)
pub mod pooled;
/// Types for transaction requests.
pub mod request;
/// Types for signed transactions.
pub mod signed;
/// The L1 transaction type.
pub mod r#type;

// Re-export the transaction types from `edr_eth`, since they are used by
// Ethereum L1.
pub use edr_eth::transaction::*;

/// Convenience type alias for [`self::pooled::L1PooledTransaction`].
///
/// This allows usage like [`edr_chain_l1::Pooled`].
pub type Pooled = self::pooled::L1PooledTransaction;

/// Convenience type alias for [`self::request::L1TransactionRequest`].
///
/// This allows usage like [`edr_chain_l1::Request`].
pub type Request = self::request::L1TransactionRequest;

/// Convenience type alias for [`self::signed::L1SignedTransaction`].
///
/// This allows usage like [`edr_chain_l1::Signed`].
pub type Signed = self::signed::L1SignedTransaction;

/// Convenience type alias for [`self::r#type::L1TransactionType`].
///
/// This allows usage like [`edr_chain_l1::Type`].
pub type Type = self::r#type::L1TransactionType;
