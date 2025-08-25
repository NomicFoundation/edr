use alloy_rlp::BufMut;

use super::{
    Recoverable, RecoveryMessage, Signature, SignatureError, SignatureWithRecoveryId,
    SignatureWithYParity, SignatureWithYParityArgs,
};
use crate::{Address, U256};

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

/// A fakeable signature which can either be a fake signature or a real ECDSA
/// signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FakeableSignature<SignatureT: Signature> {
    data: FakeableData<SignatureT>,
    address: Address,
}

impl<SignatureT: Signature> FakeableSignature<SignatureT> {
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

impl<SignatureT: Recoverable + Signature> FakeableSignature<SignatureT> {
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
    for FakeableSignature<SignatureT>
{
    fn encode(&self, out: &mut dyn BufMut) {
        match &self.data {
            FakeableData::Fake { recovery_id } => {
                if let Some(y_parity) = self.y_parity() {
                    SignatureWithYParity::new(SignatureWithYParityArgs {
                        r: self.r(),
                        s: self.s(),
                        y_parity,
                    })
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
                    SignatureWithYParity::new(SignatureWithYParityArgs {
                        r: self.r(),
                        s: self.s(),
                        y_parity,
                    })
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

impl<SignatureT: Recoverable + Signature> serde::Serialize for FakeableSignature<SignatureT> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use revm_primitives::alloy_primitives::U64;
        use serde::ser::SerializeMap;

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

impl<SignatureT: Recoverable + Signature> Signature for FakeableSignature<SignatureT> {
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
