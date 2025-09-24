use alloy_primitives::Signature as PrimitiveSignature;
use alloy_rlp::BufMut;
use k256::SecretKey;

use super::{Recoverable, RecoveryMessage, Signature, SignatureError, SignatureWithRecoveryId};
use crate::{Address, U256};

#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
/// An ECDSA signature with Y-parity.
pub struct SignatureWithYParity(PrimitiveSignature);

/// Arguments for constructing a new `SignatureWithYParity`.
pub struct Args {
    /// The `r` value of the signature.
    pub r: U256,
    /// The `s` value of the signature.
    pub s: U256,
    /// The Y-parity of the signature.
    pub y_parity: bool,
}

impl SignatureWithYParity {
    /// Constructs a new instance from the provided `r`, `s`, and `y_parity`
    /// values.
    pub fn new(args: Args) -> Self {
        let Args { r, s, y_parity } = args;

        Self(PrimitiveSignature::new(r, s, y_parity))
    }

    /// Constructs a new instance from a message and secret key.
    ///
    /// To obtain the hash of a message consider
    /// [`crate::utils::hash_message`].
    pub fn with_message<M>(message: M, secret_key: &SecretKey) -> Result<Self, SignatureError>
    where
        M: Into<RecoveryMessage>,
    {
        SignatureWithRecoveryId::new(message, secret_key).map(SignatureWithYParity::from)
    }

    /// Returns the inner `PrimitiveSignature`.
    pub const fn into_inner(self) -> PrimitiveSignature {
        self.0
    }
}

// We need a custom implementation to avoid the struct being treated as an RLP
// list.
impl alloy_rlp::Decodable for SignatureWithYParity {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        PrimitiveSignature::decode_rlp_vrs(buf, bool::decode).map(Self)
    }
}

// We need a custom implementation to avoid the struct being treated as an RLP
// list.
impl alloy_rlp::Encodable for SignatureWithYParity {
    fn encode(&self, out: &mut dyn BufMut) {
        self.0.write_rlp_vrs(out, self.0.v());
    }

    fn length(&self) -> usize {
        self.0.rlp_rs_len() + self.0.v().length()
    }
}

impl From<SignatureWithRecoveryId> for SignatureWithYParity {
    fn from(value: SignatureWithRecoveryId) -> Self {
        Self(PrimitiveSignature::new(
            value.r,
            value.s,
            value.odd_y_parity(),
        ))
    }
}

impl From<SignatureWithYParity> for PrimitiveSignature {
    fn from(value: SignatureWithYParity) -> Self {
        value.0
    }
}

impl Recoverable for SignatureWithYParity {
    fn recover_address(&self, message: RecoveryMessage) -> Result<Address, SignatureError> {
        let ecdsa = SignatureWithRecoveryId {
            r: self.0.r(),
            s: self.0.s(),
            v: u64::from(self.0.v()),
        };

        ecdsa.recover(message)
    }
}

impl Signature for SignatureWithYParity {
    fn r(&self) -> U256 {
        self.0.r()
    }

    fn s(&self) -> U256 {
        self.0.s()
    }

    fn v(&self) -> u64 {
        u64::from(self.0.v())
    }

    fn y_parity(&self) -> Option<bool> {
        Some(self.0.v())
    }
}
