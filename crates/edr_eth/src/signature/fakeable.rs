use alloy_rlp::BufMut;
use once_cell::sync::OnceCell;

use super::{
    Ecdsa, EcdsaWithYParity, Fakeable, FakeableData, Recoverable, RecoveryMessage, Signature,
    SignatureError,
};
use crate::{Address, U256, U64};

impl<SignatureT: Recoverable + Signature> Fakeable<SignatureT> {
    /// Constructs an instance with a signature that has a recoverable address.
    pub fn recoverable(signature: SignatureT) -> Self {
        Self {
            data: FakeableData::Recoverable { signature },
            address: OnceCell::new(),
        }
    }

    /// Constructs an instance with a signature that has a recoverable address,
    /// as well as that address.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the address matches the signature's
    /// recoverable address.
    pub const unsafe fn with_address(signature: SignatureT, address: Address) -> Self {
        Self {
            data: FakeableData::Recoverable { signature },
            address: OnceCell::with_value(address),
        }
    }

    /// Constructs an instance with a fake signature based on the caller's
    /// address and an optional `v` value. When no `v` value is provided, we
    /// default to 1.
    ///
    /// Recovery id (i.e. `v` value) for fake signatures is unsupported, so we
    /// always set it to the one that Hardhat is using.
    ///
    /// Hardhat legacy transactions use `v` value 0. EIP-155 transactions use a
    /// chain ID-based `v` value. From EIP-2930 transactions onwards, Hardhat
    /// uses `v` value 1.
    ///
    /// We add the +27 magic number that originates from Bitcoin as the
    /// `Signature::new` function adds it as well.
    pub fn fake(caller: Address, v: Option<u64>) -> Self {
        Self {
            data: FakeableData::Fake {
                v: v.unwrap_or(1u64) + 27,
            },
            address: OnceCell::with_value(caller),
        }
    }

    /// Whether the signature is from an impersonated account.
    pub fn is_fake(&self) -> bool {
        matches!(self.data, FakeableData::Fake { .. })
    }

    /// Recovers the Ethereum address which was used to sign the transaction.
    pub fn recover_address<MessageT>(&self, message: MessageT) -> Result<&Address, SignatureError>
    where
        MessageT: Into<RecoveryMessage>,
    {
        self.address.get_or_try_init(|| {
            let message = message.into();

            match &self.data {
                FakeableData::Recoverable { signature } => signature.recover_address(message),
                FakeableData::Fake { .. } => {
                    unreachable!("fake signature must be initialized with address")
                }
            }
        })
    }
}

// We always assume that a decoded signature is a recoverable signature.
impl<SignatureT> alloy_rlp::Decodable for Fakeable<SignatureT>
where
    SignatureT: alloy_rlp::Decodable + Recoverable + Signature,
{
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let signature = SignatureT::decode(buf)?;

        Ok(Self {
            data: FakeableData::Recoverable { signature },
            address: OnceCell::new(),
        })
    }
}

// We need a custom implementation to avoid the struct being treated as an RLP
// list.
impl<SignatureT: alloy_rlp::Encodable + Recoverable + Signature> alloy_rlp::Encodable
    for Fakeable<SignatureT>
{
    fn encode(&self, out: &mut dyn BufMut) {
        match &self.data {
            FakeableData::Fake { v } => {
                if let Some(y_parity) = self.y_parity() {
                    EcdsaWithYParity {
                        r: self.r(),
                        s: self.s(),
                        y_parity,
                    }
                    .encode(out);
                } else {
                    let ecdsa = Ecdsa {
                        r: self.r(),
                        s: self.s(),
                        v: *v,
                    };

                    println!("ecdsa: {ecdsa:?}");

                    ecdsa.encode(out);
                }
            }
            FakeableData::Recoverable { signature } => signature.encode(out),
        }
    }

    fn length(&self) -> usize {
        match &self.data {
            FakeableData::Fake { v } => {
                let v_length = if let Some(y_parity) = self.y_parity() {
                    y_parity.length()
                } else {
                    v.length()
                };

                self.r().length() + self.s().length() + v_length
            }
            FakeableData::Recoverable { signature } => signature.length(),
        }
    }
}

impl<SignatureT: Recoverable + Signature> PartialEq for Fakeable<SignatureT> {
    fn eq(&self, other: &Self) -> bool {
        self.r() == other.r() && self.s() == other.s() && self.v() == other.v()
    }
}

impl<SignatureT: Recoverable + Signature> Eq for Fakeable<SignatureT> {}

#[cfg(feature = "serde")]
impl<SignatureT: Recoverable + Signature> serde::Serialize for Fakeable<SignatureT> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;

        let mut map = serializer.serialize_map(Some(3))?;
        map.serialize_entry("r", &self.r())?;
        map.serialize_entry("s", &self.s())?;
        // We serialize the `v` field for backwards compatibility
        map.serialize_entry("v", &self.v())?;

        if let Some(y_parity) = self.y_parity() {
            map.serialize_entry("y_parity", &U64::from(y_parity))?;
        }
        map.end()
    }
}

impl<SignatureT: Recoverable + Signature> Signature for Fakeable<SignatureT> {
    fn r(&self) -> U256 {
        match &self.data {
            // We interpret the hash as a big endian U256 value.
            FakeableData::Fake { .. } => {
                // SAFETY: The address is always initialized for fake signatures.
                let address = unsafe { self.address.get_unchecked() };
                U256::try_from_be_slice(address.as_slice())
                    .expect("address is 20 bytes which fits into U256")
            }
            FakeableData::Recoverable { signature } => signature.r(),
        }
    }

    fn s(&self) -> U256 {
        match &self.data {
            // We interpret the hash as a big endian U256 value.
            FakeableData::Fake { .. } => {
                // SAFETY: The address is always initialized for fake signatures.
                let address = unsafe { self.address.get_unchecked() };
                U256::try_from_be_slice(address.as_slice())
                    .expect("address is 20 bytes which fits into U256")
            }
            FakeableData::Recoverable { signature } => signature.s(),
        }
    }

    fn v(&self) -> u64 {
        match &self.data {
            FakeableData::Fake { v } => *v,
            FakeableData::Recoverable { signature } => signature.v(),
        }
    }

    fn y_parity(&self) -> Option<bool> {
        match &self.data {
            FakeableData::Fake { v } => {
                // We add the +27 magic number that originates from Bitcoin as the
                // `Signature::new` function adds it as well.
                if *v == 28 {
                    Some(true)
                } else {
                    None
                }
            }
            FakeableData::Recoverable { signature } => signature.y_parity(),
        }
    }
}
