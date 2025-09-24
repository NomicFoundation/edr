// Part of this code was adapted from ethers-rs and is distributed under their
// licenss:
// - https://github.com/gakonst/ethers-rs/blob/cba6f071aedafb766e82e4c2f469ed5e4638337d/LICENSE-APACHE
// - https://github.com/gakonst/ethers-rs/blob/cba6f071aedafb766e82e4c2f469ed5e4638337d/LICENSE-MIT
// For the original context see: https://github.com/gakonst/ethers-rs/blob/cba6f071aedafb766e82e4c2f469ed5e4638337d/ethers-core/src/types/signature.rs

//! Ethereum signature types

mod fakeable;
mod recovery_id;
pub mod utils;
mod y_parity;

use edr_primitives::{Address, B256, U256};
pub use k256::SecretKey;
use k256::{elliptic_curve::sec1::ToEncodedPoint, FieldBytes, PublicKey};
use sha3::{Digest, Keccak256};

pub use self::{
    fakeable::FakeableSignature,
    recovery_id::SignatureWithRecoveryId,
    y_parity::{Args as SignatureWithYParityArgs, SignatureWithYParity},
};

/// Trait for signing a transaction request with a fake signature.
pub trait FakeSign {
    /// The type of the signed transaction.
    type Signed;

    /// Signs the transaction with a fake signature.
    fn fake_sign(self, sender: Address) -> Self::Signed;
}

pub trait Sign {
    /// The type of the signed transaction.
    type Signed;

    /// Signs the transaction with the provided secret key, belonging to the
    /// provided sender's address.
    ///
    /// # Safety
    ///
    /// The `caller` and `secret_key` must correspond to the same account.
    unsafe fn sign_for_sender_unchecked(
        self,
        secret_key: &SecretKey,
        caller: Address,
    ) -> Result<Self::Signed, SignatureError>;
}

/// An error involving a signature.
#[derive(Debug, thiserror::Error)]
pub enum SignatureError {
    /// Invalid length, ECDSA secp256k1 signatures with recovery are 65 bytes
    #[error("invalid signature length, got {0}, expected 65")]
    InvalidLength(usize),
    /// Invalid secret key.
    #[error("Expected 32 byte secret key")]
    InvalidSecretKeyLength,
    /// When parsing a secret key from string to hex
    #[error("Invalid hex")]
    InvalidSecretKeyHex,
    /// When parsing a signature from string to hex
    #[error(transparent)]
    DecodingError(#[from] hex::FromHexError),
    /// Thrown when signature verification failed (i.e. when the address that
    /// produced the signature did not match the expected address)
    #[error("Signature verification failed. Expected {0}, got {1}")]
    VerificationError(Address, Address),
    /// ECDSA error
    #[error(transparent)]
    ECDSAError(#[from] k256::ecdsa::signature::Error),
    /// Elliptic curve error
    #[error(transparent)]
    EllipticCurveError(#[from] k256::elliptic_curve::Error),
    /// Error in recovering public key from signature
    #[error("Public key recovery error")]
    RecoveryError,
}

/// Trait for an ECDSA signature.
pub trait Signature {
    /// Returns the signature's R-value.
    fn r(&self) -> U256;

    /// Returns the signature's S-value.
    fn s(&self) -> U256;

    /// Returns the signature's V-value.
    fn v(&self) -> u64;

    /// Signals whether the signature internally uses a boolean Y-parity instead
    /// of the V-value.
    ///
    /// This applies to EIP-2930 and later transaction signatures.
    fn y_parity(&self) -> Option<bool>;
}

/// Trait for a signature with a recoverable address.
pub trait Recoverable {
    /// Recovers the Ethereum address which was used to sign the message.
    fn recover_address(&self, message: RecoveryMessage) -> Result<Address, SignatureError>;
}

/// Recovery message data.
///
/// The message data can either be a binary message that is first hashed
/// according to EIP-191 and then recovered based on the signature or a
/// precomputed hash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecoveryMessage {
    /// Message bytes
    Data(Vec<u8>),
    /// Message hash
    Hash(B256),
}

/// Converts a [`PublicKey`] to an [`Address`].
pub fn public_key_to_address(public_key: PublicKey) -> Address {
    let public_key = public_key.to_encoded_point(/* compress = */ false);
    // First byte is header value
    let pk_bytes = public_key
        .as_bytes()
        .get(1..)
        .expect("uncompressed public key is 65 bytes");
    let hash = Keccak256::digest(pk_bytes);
    // Only take the lower 160 bits of the hash
    let hash_slice = hash.get(12..).expect("hash is 32 bytes");
    Address::from_slice(hash_slice)
}

/// It's dangerous to represent secret keys as native string types, because the
/// native string types have debug, display and serialization implementations
/// that can result in the secrets accidentally leaking to logs. It's marked as
/// deprecated, because it should be only created in exactly one place in the
/// production code.
#[deprecated]
pub struct DangerousSecretKeyStr<'a>(pub &'a str);

// It's marked as deprecated to be thoughtful abouts its usage.
#[allow(deprecated)]
/// Converts a hex string to a secret key.
pub fn secret_key_from_str(
    secret_key: DangerousSecretKeyStr<'_>,
) -> Result<SecretKey, SignatureError> {
    #[allow(deprecated)]
    let str_key = secret_key.0;
    let secret_key = if let Some(stripped) = str_key.strip_prefix("0x") {
        hex::decode(stripped)
    } else {
        hex::decode(str_key)
    }
    // Hex error can leak character, so use opaque one.
    .map_err(|_err| SignatureError::InvalidSecretKeyHex)?;
    let secret_key = FieldBytes::from_exact_iter(secret_key.into_iter())
        .ok_or_else(|| SignatureError::InvalidSecretKeyLength)?;
    SecretKey::from_bytes(&secret_key).map_err(SignatureError::EllipticCurveError)
}
