use alloy_rlp::BufMut;

use super::{
    Fakeable, FakeableData, Recoverable, RecoveryMessage, Signature, SignatureError,
    SignatureWithRecoveryId, SignatureWithYParity,
};
use crate::{Address, U256};

impl<SignatureT: Signature> Fakeable<SignatureT> {
    /// Constructs an instance with a signature that has a recoverable address,
    /// as well as that address.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the address matches the signature's
    /// recoverable address.
    pub const unsafe fn with_address_unchecked(signature: SignatureT, address: Address) -> Self {
        Self {
            data: FakeableData::Recoverable { signature },
            address,
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
    pub fn fake(address: Address, recovery_id: Option<u64>) -> Self {
        Self {
            data: FakeableData::Fake {
                recovery_id: recovery_id.unwrap_or(1u64) + 27,
            },
            address,
        }
    }

    /// Whether the signature is from an impersonated account.
    pub fn is_fake(&self) -> bool {
        matches!(self.data, FakeableData::Fake { .. })
    }

    /// Returns the Ethereum address of the transaction's caller.
    pub fn caller(&self) -> &Address {
        &self.address
    }
}

impl<SignatureT: Recoverable + Signature> Fakeable<SignatureT> {
    /// Constructs an instance with a signature that has a recoverable address.
    pub fn recover(
        signature: SignatureT,
        message: RecoveryMessage,
    ) -> Result<Self, SignatureError> {
        let address = signature.recover_address(message)?;

        Ok(Self {
            data: FakeableData::Recoverable { signature },
            address,
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
            FakeableData::Fake { recovery_id } => {
                if let Some(y_parity) = self.y_parity() {
                    SignatureWithYParity {
                        r: self.r(),
                        s: self.s(),
                        y_parity,
                    }
                    .encode(out);
                } else {
                    let ecdsa = SignatureWithRecoveryId {
                        r: self.r(),
                        s: self.s(),
                        v: *recovery_id,
                    };

                    ecdsa.encode(out);
                }
            }
            FakeableData::Recoverable { signature } => signature.encode(out),
        }
    }

    fn length(&self) -> usize {
        match &self.data {
            FakeableData::Fake { recovery_id } => {
                if let Some(y_parity) = self.y_parity() {
                    SignatureWithYParity {
                        r: self.r(),
                        s: self.s(),
                        y_parity,
                    }
                    .length()
                } else {
                    SignatureWithRecoveryId {
                        r: self.r(),
                        s: self.s(),
                        v: *recovery_id,
                    }
                    .length()
                }
            }
            FakeableData::Recoverable { signature } => signature.length(),
        }
    }
}

impl<SignatureT: Recoverable + Signature + PartialEq> PartialEq for Fakeable<SignatureT> {
    fn eq(&self, other: &Self) -> bool {
        match (&self.data, &other.data) {
            (
                FakeableData::Fake {
                    recovery_id: recovery_id1,
                },
                FakeableData::Fake {
                    recovery_id: recovery_id2,
                },
            ) => recovery_id1 == recovery_id2 && self.address == other.address,
            (
                FakeableData::Recoverable { signature: s1 },
                FakeableData::Recoverable { signature: s2 },
            ) => s1 == s2,
            _ => false,
        }
    }
}

impl<SignatureT: Recoverable + Signature + PartialEq> Eq for Fakeable<SignatureT> {}

#[cfg(feature = "serde")]
impl<SignatureT: Recoverable + Signature> serde::Serialize for Fakeable<SignatureT> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;

        use crate::U64;

        let mut map = serializer.serialize_map(Some(3))?;
        map.serialize_entry("r", &self.r())?;
        map.serialize_entry("s", &self.s())?;
        // Match geth's behavior by always serializing V-value, even when the Y-parity
        // is known.
        // <https://github.com/ethereum/go-ethereum/blob/6a49d13c13d967dd9fb2190fd110ef6d90fc09cd/core/types/transaction_marshalling.go#L81>
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
            FakeableData::Fake { .. } => U256::try_from_be_slice(self.address.as_slice())
                .expect("address is 20 bytes which fits into U256"),
            FakeableData::Recoverable { signature } => signature.r(),
        }
    }

    fn s(&self) -> U256 {
        match &self.data {
            // We interpret the hash as a big endian U256 value.
            FakeableData::Fake { .. } => U256::try_from_be_slice(self.address.as_slice())
                .expect("address is 20 bytes which fits into U256"),
            FakeableData::Recoverable { signature } => signature.s(),
        }
    }

    fn v(&self) -> u64 {
        match &self.data {
            FakeableData::Fake { recovery_id } => *recovery_id,
            FakeableData::Recoverable { signature } => signature.v(),
        }
    }

    fn y_parity(&self) -> Option<bool> {
        match &self.data {
            FakeableData::Fake { recovery_id } => {
                // We add the +27 magic number that originates from Bitcoin as the
                // `Signature::new` function adds it as well.
                if *recovery_id == 28 {
                    Some(true)
                } else {
                    None
                }
            }
            FakeableData::Recoverable { signature } => signature.y_parity(),
        }
    }
}
