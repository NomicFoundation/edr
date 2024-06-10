use alloy_rlp::BufMut;
use k256::SecretKey;

use super::{
    Fakeable, Recoverable, RecoveryMessage, Signature, SignatureError, SignatureWithRecoveryId,
};
use crate::{Address, U256};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// An ECDSA signature with Y-parity.
pub struct SignatureWithYParity {
    /// R value
    pub r: U256,
    /// S value
    pub s: U256,
    /// Whether the V value has odd Y parity.
    pub y_parity: bool,
}

impl SignatureWithYParity {
    /// Constructs a new instance from a message and secret key.
    ///
    /// To obtain the hash of a message consider [`hash_message`].
    pub fn new<M>(message: M, secret_key: &SecretKey) -> Result<Self, SignatureError>
    where
        M: Into<RecoveryMessage>,
    {
        SignatureWithRecoveryId::new(message, secret_key).map(SignatureWithYParity::from)
    }
}

// We need a custom implementation to avoid the struct being treated as an RLP
// list.
impl alloy_rlp::Decodable for SignatureWithYParity {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let decoded = Self {
            // The order of these fields determines decoding order.
            y_parity: bool::decode(buf)?,
            r: U256::decode(buf)?,
            s: U256::decode(buf)?,
        };

        Ok(decoded)
    }
}

// We need a custom implementation to avoid the struct being treated as an RLP
// list.
impl alloy_rlp::Encodable for SignatureWithYParity {
    fn encode(&self, out: &mut dyn BufMut) {
        // The order of these fields determines decoding order.
        self.y_parity.encode(out);
        self.r.encode(out);
        self.s.encode(out);
    }

    fn length(&self) -> usize {
        self.r.length() + self.s.length() + self.y_parity.length()
    }
}

impl From<SignatureWithRecoveryId> for SignatureWithYParity {
    fn from(value: SignatureWithRecoveryId) -> Self {
        Self {
            r: value.r,
            s: value.s,
            y_parity: value.odd_y_parity(),
        }
    }
}

impl From<SignatureWithYParity> for Fakeable<SignatureWithYParity> {
    fn from(value: SignatureWithYParity) -> Self {
        Self::recoverable(value)
    }
}

impl Recoverable for SignatureWithYParity {
    fn recover_address(&self, message: RecoveryMessage) -> Result<Address, SignatureError> {
        let ecdsa = SignatureWithRecoveryId {
            r: self.r,
            s: self.s,
            v: self.v(),
        };

        ecdsa.recover(message)
    }
}

impl Signature for SignatureWithYParity {
    fn r(&self) -> U256 {
        self.r
    }

    fn s(&self) -> U256 {
        self.s
    }

    fn v(&self) -> u64 {
        u64::from(self.y_parity)
    }

    fn y_parity(&self) -> Option<bool> {
        Some(self.y_parity)
    }
}
