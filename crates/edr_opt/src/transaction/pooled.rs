use edr_eth::transaction::{
    pooled::Eip4844PooledTransaction, Eip1559SignedTransaction, Eip155SignedTransaction,
    Eip2930SignedTransaction, LegacySignedTransaction,
};

use super::deposited;

/// An Optimism pooled transaction, used to communicate between node pools.
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
    Eip4844(Eip4844PooledTransaction),
    /// Optimism deposited transaction
    Deposited(deposited::Transaction),
}
