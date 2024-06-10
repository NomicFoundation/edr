use std::fmt::Debug;

use edr_eth::transaction::SignedTransaction;
use revm::primitives::TxEnv;

/// A trait for defining a chain's associated types.
pub trait ChainSpec {
    /// The type of signed transactions used by this chain.
    type SignedTransaction: alloy_rlp::Encodable
        + Clone
        + Debug
        + TryInto<TxEnv>
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
