use std::sync::OnceLock;

use crate::{
    signature::{Fakeable, SignatureWithYParity},
    B256,
};

/// Trait for computing the hash of a transaction.
pub trait ComputeTransactionHash {
    /// Computes the hash of the transaction.
    fn compute_transaction_hash(&self) -> B256;
}

/// A sealed transaction.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Sealed<TransactionT: ComputeTransactionHash> {
    transaction: TransactionT,
    signature: Fakeable<SignatureWithYParity>,
    #[cfg_attr(feature = "serde", serde(skip))]
    hash: OnceLock<B256>,
}

impl<TransactionT: ComputeTransactionHash> Sealed<TransactionT> {
    pub fn signature(&self) -> &Fakeable<SignatureWithYParity> {
        &self.signature
    }

    pub fn transaction(&self) -> &TransactionT {
        &self.transaction
    }

    pub fn transaction_hash(&self) -> &B256 {
        self.hash
            .get_or_init(|| self.transaction.compute_transaction_hash())
    }
}

impl<TransactionT: ComputeTransactionHash + PartialEq> PartialEq for Sealed<TransactionT> {
    fn eq(&self, other: &Self) -> bool {
        self.transaction == other.transaction && self.signature == other.signature
    }
}

impl<TransactionT: ComputeTransactionHash + Eq> Eq for Sealed<TransactionT> {}
