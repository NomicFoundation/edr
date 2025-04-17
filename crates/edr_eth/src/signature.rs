// Part of this code was adapted from ethers-rs and is distributed under their
// licenss:
// - https://github.com/gakonst/ethers-rs/blob/cba6f071aedafb766e82e4c2f469ed5e4638337d/LICENSE-APACHE
// - https://github.com/gakonst/ethers-rs/blob/cba6f071aedafb766e82e4c2f469ed5e4638337d/LICENSE-MIT
// For the original context see: https://github.com/gakonst/ethers-rs/blob/cba6f071aedafb766e82e4c2f469ed5e4638337d/ethers-core/src/types/signature.rs

mod fakeable;
mod recovery_id;
mod y_parity;

pub use k256::SecretKey;
use k256::{FieldBytes, PublicKey, elliptic_curve::sec1::ToEncodedPoint};
use sha3::{Digest, Keccak256};

pub use self::{
    recovery_id::SignatureWithRecoveryId,
    y_parity::{Args as SignatureWithYParityArgs, SignatureWithYParity},
};
use crate::{Address, B256, U256};

/// An error involving a signature.
#[derive(Debug)]
#[cfg_attr(feature = "std", derive(thiserror::Error))]
pub enum SignatureError {
    /// Invalid length, ECDSA secp256k1 signatures with recovery are 65 bytes
    #[cfg_attr(
        feature = "std",
        error("invalid signature length, got {0}, expected 65")
    )]
    InvalidLength(usize),
    /// Invalid secret key.
    #[cfg_attr(feature = "std", error("Expected 32 byte secret key"))]
    InvalidSecretKeyLength,
    /// When parsing a secret key from string to hex
    #[cfg_attr(feature = "std", error("Invalid hex"))]
    InvalidSecretKeyHex,
    /// When parsing a signature from string to hex
    #[cfg_attr(feature = "std", error(transparent))]
    DecodingError(#[cfg_attr(feature = "std", from)] hex::FromHexError),
    /// Thrown when signature verification failed (i.e. when the address that
    /// produced the signature did not match the expected address)
    #[cfg_attr(
        feature = "std",
        error("Signature verification failed. Expected {0}, got {1}")
    )]
    VerificationError(Address, Address),
    /// ECDSA error
    #[cfg_attr(feature = "std", error(transparent))]
    ECDSAError(#[cfg_attr(feature = "std", from)] k256::ecdsa::signature::Error),
    /// Elliptic curve error
    #[cfg_attr(feature = "std", error(transparent))]
    EllipticCurveError(#[cfg_attr(feature = "std", from)] k256::elliptic_curve::Error),
    /// Error in recovering public key from signature
    #[cfg_attr(feature = "std", error("Public key recovery error"))]
    RecoveryError,
}

/// A fakeable signature which can either be a fake signature or a real ECDSA
/// signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fakeable<SignatureT: Signature> {
    data: FakeableData<SignatureT>,
    address: Address,
}

/// Signature with a recoverable caller address.
#[derive(Clone, Debug, PartialEq, Eq)]
enum FakeableData<SignatureT: Signature> {
    /// Fake signature, used for impersonation.
    /// Contains the caller address.
    ///
    /// The only requirements on a fake signature are that when it is encoded as
    /// part of a transaction, it produces the same hash for the same
    /// transaction from a sender, and it produces different hashes for
    /// different senders. We achieve this by setting the `r` and `s` values
    /// to the sender's address. This is the simplest implementation and it
    /// helps us recognize fake signatures in debug logs.
    Fake {
        /// The fake recovery ID.
        ///
        /// A recovery ID of 28 (1 + 27) signals that the signature uses a
        /// `y_parity: bool` for encoding/decoding purposes instead of `v: u64`.
        recovery_id: u64,
    },
    /// ECDSA signature with a recoverable caller address.
    Recoverable { signature: SignatureT },
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
    let hash = Keccak256::digest(&public_key.as_bytes()[1..]);
    // Only take the lower 160 bits of the hash
    Address::from_slice(&hash[12..])
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
