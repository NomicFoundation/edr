/// Types for transaction gossip (aka pooled transactions)
pub mod pooled;
/// Types for transaction requests.
pub mod request;
/// Types for signed transactions.
pub mod signed;
/// The L1 transaction type.
pub mod r#type;

pub type Pooled = self::pooled::L1PooledTransaction;
pub type Request = self::request::L1TransactionRequest;
pub type Signed = self::signed::L1SignedTransaction;
pub type Type = self::r#type::L1TransactionType;
