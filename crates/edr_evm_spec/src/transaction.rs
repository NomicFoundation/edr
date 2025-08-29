use core::fmt::Debug;

use revm_primitives::{Address, Bytes, TxKind, B256, U256};

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
    fn access_list(&self) -> Option<&[edr_eip2930::AccessListItem]>;

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
    /// one blob present if [`crate::Transaction::max_fee_per_blob_gas`] is
    /// `Some`.
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
    fn authorization_list(&self) -> Option<&[edr_eip7702::SignedAuthorization]>;

    /// The enveloped (EIP-2718) RLP-encoding of the transaction.
    fn rlp_encoding(&self) -> &Bytes;

    /// The hash of the transaction.
    fn transaction_hash(&self) -> &B256;
}

/// Trait for validating a transaction.
pub trait TransactionValidation {
    /// An error that occurs when validating a transaction.
    type ValidationError: Debug + std::error::Error;
}
