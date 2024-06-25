/// Types for Optimism execution receipts.
pub mod execution;

use crate::transaction;

/// Optimism execution receipt.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(untagged)]
pub enum Execution<LogT> {
    /// Legacy receipt.
    Legacy(edr_eth::receipt::execution::Legacy<LogT>),
    /// EIP-658 receipt.
    Eip658(edr_eth::receipt::execution::Eip658<LogT>),
    /// EIP-2718 receipt.
    Eip2718(edr_eth::receipt::execution::Eip2718<LogT, transaction::Type>),
    /// Optimism deposit receipt (post-Regolith).
    Deposit(self::execution::Deposit<LogT>),
}
