use edr_eth::transaction;

use super::deposited;

/// An optimism signed transaction, used in blocks.
pub enum Transaction {
    /// Legacy transaction before EIP-155
    PreEip155Legacy(transaction::signed::Legacy),
    /// Legacy transaction after EIP-155
    PostEip155Legacy(transaction::signed::Eip155),
    /// EIP-2930 transaction
    Eip2930(transaction::signed::Eip2930),
    /// EIP-1559 transaction
    Eip1559(transaction::signed::Eip1559),
    /// EIP-4844 transaction
    Eip4844(transaction::signed::Eip4844),
    /// Optimism deposited transaction
    Deposited(deposited::Transaction),
}
