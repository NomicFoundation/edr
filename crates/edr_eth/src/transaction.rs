// Part of this code was inspired by foundry. For the original context see:
// https://github.com/foundry-rs/foundry/blob/01b16238ff87dc7ca8ee3f5f13e389888c2a2ee4/anvil/core/src/eth/transaction/mod.rs
#![allow(missing_docs)]

//! transaction related data

mod fake_signature;
/// Types for transaction gossip (aka pooled transactions)
pub mod pooled;
/// Types for transaction requests.
pub mod request;
/// Types for signed transactions.
pub mod signed;

use core::fmt::Debug;

pub use revm_context_interface::Transaction;
pub use revm_primitives::alloy_primitives::TxKind;
use revm_primitives::{ruint, B256};

use crate::{
    eips::{eip2930, eip7702},
    signature::Signature,
    Address, Bytes, U256,
};

pub const INVALID_TX_TYPE_ERROR_MESSAGE: &str = "invalid tx type";

/// Trait for computing the hash of a transaction.
pub trait ComputeTransactionHash {
    /// Computes the hash of the transaction.
    fn compute_transaction_hash(&self) -> B256;
}

impl From<ruint::ParseError> for ParseError {
    fn from(error: ruint::ParseError) -> Self {
        match error {
            ruint::ParseError::InvalidDigit(c) => ParseError::InvalidDigit(c),
            ruint::ParseError::InvalidRadix(_) => ParseError::InvalidRadix,
            ruint::ParseError::BaseConvertError(error) => ParseError::BaseConvertError(error),
        }
    }
}

/// Trait for information about executable transactions.
pub trait ExecutableTransaction {
    /// Caller aka Author aka transaction signer.
    fn caller(&self) -> &Address;

    /// The maximum amount of gas the transaction can use.
    fn gas_limit(&self) -> u64;

    /// The gas price the sender is willing to pay.
    fn gas_price(&self) -> &u128;

    /// Returns what kind of transaction this is.
    fn kind(&self) -> TxKind;

    /// The value sent to the receiver of `TxKind::Call`.
    fn value(&self) -> &U256;

    /// Returns the input data of the transaction.
    fn data(&self) -> &Bytes;

    /// The nonce of the transaction.
    fn nonce(&self) -> u64;

    /// The chain ID of the transaction. If set to `None`, no checks are
    /// performed.
    ///
    /// Incorporated as part of the Spurious Dragon upgrade via [EIP-155].
    ///
    /// [EIP-155]: https://eips.ethereum.org/EIPS/eip-155
    fn chain_id(&self) -> Option<u64>;

    /// A list of addresses and storage keys that the transaction plans to
    /// access.
    ///
    /// Added in [EIP-2930].
    ///
    /// [EIP-2930]: https://eips.ethereum.org/EIPS/eip-2930
    fn access_list(&self) -> Option<&[eip2930::AccessListItem]>;

    /// The effective gas price of the transaction, calculated using the
    /// provided block base fee. Only applicable for post-EIP-1559 transactions.
    fn effective_gas_price(&self, block_base_fee: u128) -> Option<u128>;

    /// The maximum fee per gas the sender is willing to pay. Only applicable
    /// for post-EIP-1559 transactions.
    fn max_fee_per_gas(&self) -> Option<&u128>;

    /// The maximum priority fee per gas the sender is willing to pay.
    ///
    /// Incorporated as part of the London upgrade via [EIP-1559].
    ///
    /// [EIP-1559]: https://eips.ethereum.org/EIPS/eip-1559
    fn max_priority_fee_per_gas(&self) -> Option<&u128>;

    /// The list of blob versioned hashes. Per EIP there should be at least
    /// one blob present if [`Transaction::max_fee_per_blob_gas`] is `Some`.
    ///
    /// Incorporated as part of the Cancun upgrade via [EIP-4844].
    ///
    /// [EIP-4844]: https://eips.ethereum.org/EIPS/eip-4844
    fn blob_hashes(&self) -> &[B256];

    /// The maximum fee per blob gas the sender is willing to pay.
    ///
    /// Incorporated as part of the Cancun upgrade via [EIP-4844].
    ///
    /// [EIP-4844]: https://eips.ethereum.org/EIPS/eip-4844
    fn max_fee_per_blob_gas(&self) -> Option<&u128>;

    /// The total amount of blob gas used by the transaction. Only applicable
    /// for EIP-4844 transactions.
    fn total_blob_gas(&self) -> Option<u64>;

    /// List of authorizations, that contains the signature that authorizes this
    /// caller to place the code to signer account.
    ///
    /// Set EOA account code for one transaction
    ///
    /// [EIP-Set EOA account code for one transaction](https://eips.ethereum.org/EIPS/eip-7702)
    fn authorization_list(&self) -> Option<&[eip7702::SignedAuthorization]>;

    /// The enveloped (EIP-2718) RLP-encoding of the transaction.
    fn rlp_encoding(&self) -> &Bytes;

    /// The hash of the transaction.
    fn transaction_hash(&self) -> &B256;
}

/// Macro for implementing [`revm_context_interface::Transaction`] for a type
/// using the existing implementations of [`ExecutableTransaction`] and
/// [`TransactionType`].
#[macro_export]
macro_rules! impl_revm_transaction_trait {
    ($ty:ty) => {
        impl $crate::transaction::Transaction for $ty {
            type AccessListItem = $crate::eips::eip2930::AccessListItem;
            type Authorization = $crate::eips::eip7702::SignedAuthorization;

            fn tx_type(&self) -> u8 {
                $crate::transaction::TransactionType::transaction_type(self).into()
            }

            fn caller(&self) -> $crate::Address {
                $crate::transaction::ExecutableTransaction::caller(self).clone()
            }
            fn gas_limit(&self) -> u64 {
                $crate::transaction::ExecutableTransaction::gas_limit(self)
            }

            fn value(&self) -> $crate::U256 {
                $crate::transaction::ExecutableTransaction::value(self).clone()
            }

            fn input(&self) -> &$crate::Bytes {
                $crate::transaction::ExecutableTransaction::data(self)
            }

            fn nonce(&self) -> u64 {
                $crate::transaction::ExecutableTransaction::nonce(self)
            }

            fn kind(&self) -> $crate::transaction::TxKind {
                $crate::transaction::ExecutableTransaction::kind(self)
            }

            fn chain_id(&self) -> Option<u64> {
                $crate::transaction::ExecutableTransaction::chain_id(self)
            }

            fn gas_price(&self) -> u128 {
                $crate::transaction::ExecutableTransaction::gas_price(self).clone()
            }

            fn access_list(&self) -> Option<impl Iterator<Item = &Self::AccessListItem>> {
                $crate::transaction::ExecutableTransaction::access_list(self)
                    .map(|list| list.iter())
            }

            fn blob_versioned_hashes(&self) -> &[$crate::B256] {
                $crate::transaction::ExecutableTransaction::blob_hashes(self)
            }

            fn max_fee_per_blob_gas(&self) -> u128 {
                $crate::transaction::ExecutableTransaction::max_fee_per_blob_gas(self)
                    .cloned()
                    .unwrap_or(0u128)
            }

            fn authorization_list_len(&self) -> usize {
                $crate::transaction::ExecutableTransaction::authorization_list(self)
                    .map_or(0, |list| list.len())
            }

            fn authorization_list(&self) -> impl Iterator<Item = &Self::Authorization> {
                $crate::transaction::ExecutableTransaction::authorization_list(self)
                    .unwrap_or(&[])
                    .iter()
            }

            fn max_priority_fee_per_gas(&self) -> Option<u128> {
                $crate::transaction::ExecutableTransaction::max_priority_fee_per_gas(self).cloned()
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

/// Trait for validating a transaction.
pub trait TransactionValidation {
    /// An error that occurs when validating a transaction.
    type ValidationError: Debug + std::error::Error;
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
