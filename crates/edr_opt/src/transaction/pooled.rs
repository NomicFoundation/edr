use edr_eth::transaction;

use super::deposited;

/// An Optimism pooled transaction, used to communicate between node pools.
pub enum Transaction {
    /// Legacy transaction before EIP-155
    PreEip155Legacy(transaction::pooled::Legacy),
    /// Legacy transaction after EIP-155
    PostEip155Legacy(transaction::pooled::Eip155),
    /// EIP-2930 transaction
    Eip2930(transaction::pooled::Eip2930),
    /// EIP-1559 transaction
    Eip1559(transaction::pooled::Eip1559),
    /// EIP-4844 transaction
    Eip4844(transaction::pooled::Eip4844),
    /// Optimism deposited transaction
    Deposited(deposited::Transaction),
}
