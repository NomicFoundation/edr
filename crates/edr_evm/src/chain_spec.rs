use std::fmt::Debug;

use edr_eth::transaction::SignedTransaction;

pub trait ChainSpec {
    /// The type of signed transactions used by this chain.
    type SignedTransaction: Clone + Debug + PartialEq + Eq + SignedTransaction;
}

pub struct L1ChainSpec;

impl ChainSpec for L1ChainSpec {
    type SignedTransaction = edr_eth::transaction::Signed;
}
