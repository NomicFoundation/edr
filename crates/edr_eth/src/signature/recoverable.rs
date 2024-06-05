use alloy_rlp::BufMut;
use k256::SecretKey;

use super::{Recoverable, RecoveryMessage, SignatureError};
use crate::{Address, B256, U256, U64};

impl Recoverable {
    /// Constructs an instance with a fake signature.
    pub fn fake(caller: Address, v: u64, has_y_parity: bool) -> Self {
        Self::Fake {
            caller,
            has_y_parity,
            v,
        }
    }

    /// Constructs an instance with R-, S-, and V-values.
    pub fn with_rsv(r: U256, s: U256, v: u64) -> Self {
        Self::Rsv { r, s, v }
    }

    /// Constructs an instance with R-, S-, and Y-parity values.
    pub fn rs_and_y_parity(hash: B256, secret_key: &SecretKey) -> Result<Self, SignatureError> {
        let signature = super::Ecdsa::new(hash, secret_key)?;

        Ok(Self::RsyParity {
            r: signature.r,
            s: signature.s,
            y_parity: signature.odd_y_parity(),
        })
    }

    /// Converts the instance to a [`super::Ecdsa`].
    pub fn as_ecdsa(&self) -> super::Ecdsa {
        super::Ecdsa {
            r: self.r(),
            s: self.s(),
            v: self.v(),
        }
    }

    /// Whether the signature is from an impersonated account.
    pub fn is_fake(&self) -> bool {
        matches!(self, Self::Fake { .. })
    }

    /// Recovers the Ethereum address which was used to sign the transaction.
    pub fn recover_address<MessageT>(&self, message: MessageT) -> Result<Address, SignatureError>
    where
        MessageT: Into<RecoveryMessage>,
    {
        match self {
            Self::Fake { caller, .. } => Ok(*caller),
            Self::Rsv { .. } | Self::RsyParity { .. } => self.as_ecdsa().recover(message),
        }
    }

    /// Returns the signature's R-value.
    pub fn r(&self) -> U256 {
        match self {
            // We interpret the hash as a big endian U256 value.
            Self::Fake { caller, .. } => U256::try_from_be_slice(caller.as_slice())
                .expect("address is 20 bytes which fits into U256"),
            Self::Rsv { r, .. } | Self::RsyParity { r, .. } => *r,
        }
    }

    /// Returns the signature's S-value.
    pub fn s(&self) -> U256 {
        match self {
            // We interpret the hash as a big endian U256 value.
            Self::Fake { caller, .. } => U256::try_from_be_slice(caller.as_slice())
                .expect("address is 20 bytes which fits into U256"),
            Self::Rsv { s, .. } | Self::RsyParity { s, .. } => *s,
        }
    }

    /// Returns the signature's V-value.
    pub fn v(&self) -> u64 {
        match self {
            // Recovery id for fake signatures is unsupported, so we always set it to the
            // one that Hardhat is using. We add the +27 magic number that originates
            // from Bitcoin as the `Signature::new` function adds it as well.
            Self::Fake { v, .. } => v + 27,
            Self::Rsv { v, .. } => *v,
            Self::RsyParity { y_parity, .. } => u64::from(*y_parity),
        }
    }
}

// We always assume that a decoded signature is an Ecdsa signature.
// We need a custom implementation to avoid the struct being treated as an RLP
// list.
impl alloy_rlp::Decodable for Recoverable {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let decode = Self::Rsv {
            // The order of these fields determines decoding order.
            v: u64::decode(buf)?,
            r: U256::decode(buf)?,
            s: U256::decode(buf)?,
        };

        Ok(decode)
    }
}

// We need a custom implementation to avoid the struct being treated as an RLP
// list.
impl alloy_rlp::Encodable for Recoverable {
    fn encode(&self, out: &mut dyn BufMut) {
        // The order of these fields determines decoding order.
        self.v().encode(out);
        self.r().encode(out);
        self.s().encode(out);
    }

    fn length(&self) -> usize {
        self.r().length() + self.s().length() + self.v().length()
    }
}

impl From<super::Ecdsa> for Recoverable {
    fn from(value: super::Ecdsa) -> Self {
        Self::Rsv {
            r: value.r,
            s: value.s,
            v: value.v,
        }
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Recoverable {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;

        fn serialize_with_y_parity<S: serde::Serializer>(
            serializer: S,
            r: U256,
            s: U256,
            y_parity: u64,
        ) -> Result<S::Ok, S::Error> {
            let mut map = serializer.serialize_map(Some(3))?;
            map.serialize_entry("r", &r)?;
            map.serialize_entry("s", &s)?;
            map.serialize_entry("y_parity", &U64::from(y_parity))?;
            map.end()
        }

        match self {
            Self::Fake {
                v: y_parity,
                has_y_parity,
                ..
            } if *has_y_parity => {
                serialize_with_y_parity(serializer, self.r(), self.s(), *y_parity)
            }
            Self::Fake { .. } => super::Ecdsa {
                r: self.r(),
                s: self.s(),
                v: self.v(),
            }
            .serialize(serializer),
            Self::Rsv { r, s, v } => super::Ecdsa {
                r: *r,
                s: *s,
                v: *v,
            }
            .serialize(serializer),
            Self::RsyParity { r, s, y_parity } => {
                serialize_with_y_parity(serializer, *r, *s, u64::from(*y_parity))
            }
        }
    }
}
