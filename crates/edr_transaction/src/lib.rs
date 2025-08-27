// Part of this code was inspired by foundry. For the original context see:
// https://github.com/foundry-rs/foundry/blob/01b16238ff87dc7ca8ee3f5f13e389888c2a2ee4/anvil/core/src/eth/transaction/mod.rs
#![allow(missing_docs)]

//! Ethereum transaction types

/// Types for transaction gossip (aka pooled transactions)
pub mod pooled;
/// Types for transaction requests.
pub mod request;
/// Types for signed transactions.
pub mod signed;
mod test_utils;
/// Utility functions
pub mod utils;

use edr_evm_spec::ExecutableTransaction;
use edr_signer::Signature;
pub use revm_context_interface::Transaction;
pub use revm_primitives::{
    alloy_primitives::{TxKind, U8},
    ruint::{
        aliases::U256, BaseConvertError as RuintBaseConvertError, ParseError as RuintParseError,
    },
    Address, Bytes, B256,
};

pub const INVALID_TX_TYPE_ERROR_MESSAGE: &str = "invalid tx type";

/// Trait for computing the hash of a transaction.
pub trait ComputeTransactionHash {
    /// Computes the hash of the transaction.
    fn compute_transaction_hash(&self) -> B256;
}

/// Macro for implementing [`revm_context_interface::Transaction`] for a type
/// using the existing implementations of [`ExecutableTransaction`] and
/// [`TransactionType`].
#[macro_export]
macro_rules! impl_revm_transaction_trait {
    ($ty:ty) => {
        impl $crate::Transaction for $ty {
            type AccessListItem<'a> = &'a edr_eip2930::AccessListItem;
            type Authorization<'a> = &'a edr_eip7702::SignedAuthorization;

            fn tx_type(&self) -> u8 {
                $crate::TransactionType::transaction_type(self).into()
            }

            fn caller(&self) -> $crate::Address {
                edr_evm_spec::ExecutableTransaction::caller(self).clone()
            }
            fn gas_limit(&self) -> u64 {
                edr_evm_spec::ExecutableTransaction::gas_limit(self)
            }

            fn value(&self) -> $crate::U256 {
                edr_evm_spec::ExecutableTransaction::value(self).clone()
            }

            fn input(&self) -> &$crate::Bytes {
                edr_evm_spec::ExecutableTransaction::data(self)
            }

            fn nonce(&self) -> u64 {
                edr_evm_spec::ExecutableTransaction::nonce(self)
            }

            fn kind(&self) -> $crate::TxKind {
                edr_evm_spec::ExecutableTransaction::kind(self)
            }

            fn chain_id(&self) -> Option<u64> {
                edr_evm_spec::ExecutableTransaction::chain_id(self)
            }

            fn gas_price(&self) -> u128 {
                edr_evm_spec::ExecutableTransaction::gas_price(self).clone()
            }

            fn access_list(&self) -> Option<impl Iterator<Item = Self::AccessListItem<'_>>> {
                edr_evm_spec::ExecutableTransaction::access_list(self).map(|list| list.iter())
            }

            fn blob_versioned_hashes(&self) -> &[$crate::B256] {
                edr_evm_spec::ExecutableTransaction::blob_hashes(self)
            }

            fn max_fee_per_blob_gas(&self) -> u128 {
                edr_evm_spec::ExecutableTransaction::max_fee_per_blob_gas(self)
                    .cloned()
                    .unwrap_or(0u128)
            }

            fn authorization_list_len(&self) -> usize {
                edr_evm_spec::ExecutableTransaction::authorization_list(self)
                    .map_or(0, |list| list.len())
            }

            fn authorization_list(&self) -> impl Iterator<Item = Self::Authorization<'_>> {
                edr_evm_spec::ExecutableTransaction::authorization_list(self)
                    .unwrap_or(&[])
                    .iter()
            }

            fn max_priority_fee_per_gas(&self) -> Option<u128> {
                edr_evm_spec::ExecutableTransaction::max_priority_fee_per_gas(self).cloned()
            }
        }
    };
}

/// Trait for transactions that may be signed.
pub trait MaybeSignedTransaction {
    /// Returns the [`Signature`] of the transaction, if any.
    fn maybe_signature(&self) -> Option<&dyn Signature>;
}

/// Trait for transactions that have been signed.
pub trait SignedTransaction {
    /// Returns the [`Signature`] of the transaction.
    fn signature(&self) -> &dyn Signature;
}

impl<TransactionT: SignedTransaction> MaybeSignedTransaction for TransactionT {
    fn maybe_signature(&self) -> Option<&dyn Signature> {
        Some(self.signature())
    }
}

/// Trait for mutable transactions.
pub trait TransactionMut {
    /// Sets the gas limit of the transaction.
    fn set_gas_limit(&mut self, gas_limit: u64);
}

/// Trait for determining the type of a transaction.
pub trait TransactionType {
    /// Type of the transaction.
    type Type: Into<u8>;

    /// Returns the type of the transaction.
    fn transaction_type(&self) -> Self::Type;
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("{0}")]
    BaseConvertError(RuintBaseConvertError),
    #[error("Invalid digit: {0}")]
    InvalidDigit(char),
    #[error("Invalid radix. Only hexadecimal is supported.")]
    InvalidRadix,
    #[error("Unknown transaction type: {0}")]
    UnknownType(u8),
}

impl From<RuintParseError> for ParseError {
    fn from(error: RuintParseError) -> Self {
        match error {
            RuintParseError::InvalidDigit(c) => ParseError::InvalidDigit(c),
            RuintParseError::InvalidRadix(_) => ParseError::InvalidRadix,
            RuintParseError::BaseConvertError(error) => ParseError::BaseConvertError(error),
        }
    }
}

/// Trait for determining whether a transaction is an EIP-155 transaction.
pub trait IsEip155 {
    /// Whether the transaction is an EIP-155 transaction.
    fn is_eip155(&self) -> bool;
}

/// Trait for determining whether a transaction is an EIP-4844 transaction.
pub trait IsEip4844 {
    /// Whether the transaction is an EIP-4844 transaction.
    fn is_eip4844(&self) -> bool;
}

/// Trait for determining whether a transaction is a legacy transaction.
pub trait IsLegacy {
    /// Whether the transaction is a legacy transaction.
    fn is_legacy(&self) -> bool;
}

/// Trait for determining whether a transaction is natively supported by the
/// chain. Unsupported transactions might still be executable, but can have
/// unexpected side effects.
pub trait IsSupported {
    /// Whether the transaction is natively supported.
    fn is_supported_transaction(&self) -> bool;
}

pub fn max_cost(transaction: &impl ExecutableTransaction) -> u128 {
    u128::from(transaction.gas_limit()).saturating_mul(*transaction.gas_price())
}

pub fn upfront_cost(transaction: &impl ExecutableTransaction) -> U256 {
    U256::from(max_cost(transaction)).saturating_add(*transaction.value())
}
