use edr_eth::transaction::{
    Eip1559SignedTransaction, Eip155SignedTransaction, Eip2930SignedTransaction,
    Eip4844SignedTransaction, LegacySignedTransaction,
};

use super::deposited;

/// An optimism signed transaction, used in blocks.
pub enum Transaction {
    /// Legacy transaction before EIP-155
    PreEip155Legacy(LegacySignedTransaction),
    /// Legacy transaction after EIP-155
    PostEip155Legacy(Eip155SignedTransaction),
    /// EIP-2930 transaction
    Eip2930(Eip2930SignedTransaction),
    /// EIP-1559 transaction
    Eip1559(Eip1559SignedTransaction),
    /// EIP-4844 transaction
    Eip4844(Eip4844SignedTransaction),
    /// Optimism deposited transaction
    Deposited(deposited::Transaction),
}
