pub use edr_eth::receipt::BlockReceipt;

/// Types for Optimism execution receipts.
pub mod execution;

/// Optimism execution receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Execution<LogT> {
    /// Legacy receipt.
    Legacy(edr_eth::receipt::execution::Legacy<LogT>),
    /// EIP-658 receipt.
    Eip658(edr_eth::receipt::execution::Eip658<LogT>),
    /// Optimism deposit receipt (post-Regolith).
    Deposit(self::execution::Deposit<LogT>),
}
