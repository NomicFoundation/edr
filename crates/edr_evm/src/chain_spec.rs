use std::fmt::Debug;

use edr_eth::{transaction::SignedTransaction, Address};
use revm::primitives::TxEnv;

/// A trait for defining a chain's associated types.
pub trait ChainSpec {
    /// The type of signed transactions used by this chain.
    type SignedTransaction: alloy_rlp::Encodable
        + Clone
        + Debug
        + IntoTxEnv
        + PartialEq
        + Eq
        + SignedTransaction;
}

/// The chain specification for Ethereum Layer 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct L1ChainSpec;

impl ChainSpec for L1ChainSpec {
    type SignedTransaction = edr_eth::transaction::Signed;
}

// TODO: This is a temporary solution until the revm fork has been merged. In
// the fork each chain type has its own transaction type with an accompanying
// trait.
/// A trait for converting a transaction into a [`revm::primitives::TxEnv`].
pub trait IntoTxEnv {
    /// Converts the transaction into a [`revm::primitives::TxEnv`] using the
    /// provided caller address.
    fn into_tx_env(self, caller: Address) -> TxEnv;
}

impl IntoTxEnv for edr_eth::transaction::Signed {
    fn into_tx_env(self, caller: Address) -> TxEnv {
        match self {
            Self::PreEip155Legacy(tx) => tx.into_tx_env(caller),
            Self::PostEip155Legacy(tx) => tx.into_tx_env(caller),
            Self::Eip2930(tx) => tx.into_tx_env(caller),
            Self::Eip1559(tx) => tx.into_tx_env(caller),
            Self::Eip4844(tx) => tx.into_tx_env(caller),
        }
    }
}
