use core::fmt;
#[cfg(feature = "std")]
use std::str::FromStr;

use alloy_rlp::BufMut;
use k256::{
    FieldBytes, SecretKey,
    ecdsa::{
        RecoveryId, Signature as ECDSASignature, SigningKey, VerifyingKey,
        signature::hazmat::PrehashSigner,
    },
};

use super::{Recoverable, RecoveryMessage, Signature, SignatureError, public_key_to_address};
use crate::{Address, B256, Bytes, U256, utils::hash_message};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// An ECDSA signature with recovery ID.
pub struct SignatureWithRecoveryId {
    /// R value
    pub r: U256,
    /// S Value
    pub s: U256,
    /// V value
    pub v: u64,
}

impl fmt::Display for SignatureWithRecoveryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sig = <[u8; 65]>::from(self);
        write!(f, "{}", hex::encode(&sig[..]))
    }
}

impl SignatureWithRecoveryId {
    /// Constructs a new signature from a message and secret key.
    /// To obtain the hash of a message consider [`hash_message`].
    pub fn new<M>(message: M, secret_key: &SecretKey) -> Result<Self, SignatureError>
    where
        M: Into<RecoveryMessage>,
    {
        let message = message.into();
        let message_hash = match message {
            RecoveryMessage::Data(ref message) => hash_message(message),
            RecoveryMessage::Hash(hash) => hash,
        };

        let signing_key: SigningKey = secret_key.into();
        let (signature, recovery_id) = PrehashSigner::<(ECDSASignature, RecoveryId)>::sign_prehash(
            &signing_key,
            &*message_hash,
        )
        .map_err(SignatureError::ECDSAError)?;

        let r = U256::try_from_be_slice(Into::<FieldBytes>::into(signature.r()).as_slice())
            .expect("Must be valid");
        let s = U256::try_from_be_slice(Into::<FieldBytes>::into(signature.s()).as_slice())
            .expect("Must be valid");
        let v = 27 + u64::from(Into::<u8>::into(recovery_id));

        Ok(Self { r, s, v })
    }

    /// Returns whether the V value has odd Y parity.
    pub fn odd_y_parity(&self) -> bool {
        self.v == 28
    }

    /// Verifies that signature on `message` was produced by `address`
    pub fn verify<M, A>(&self, message: M, address: A) -> Result<(), SignatureError>
    where
        M: Into<RecoveryMessage>,
        A: Into<Address>,
    {
        let address = address.into();
        let recovered = self.recover(message)?;
        if recovered != address {
            return Err(SignatureError::VerificationError(address, recovered));
        }

        Ok(())
    }

    /// Recovers the Ethereum address which was used to sign the given message.
    pub fn recover<M>(&self, message: M) -> Result<Address, SignatureError>
    where
        M: Into<RecoveryMessage>,
    {
        let message = message.into();
        let message_hash = match message {
            RecoveryMessage::Data(ref message) => hash_message(message),
            RecoveryMessage::Hash(hash) => hash,
        };

        let (signature, recovery_id) = self.as_signature()?;

        let verifying_key =
            VerifyingKey::recover_from_prehash(message_hash.as_slice(), &signature, recovery_id)
                .map_err(SignatureError::ECDSAError)?;

        Ok(public_key_to_address(verifying_key.into()))
    }

    /// Retrieves the recovery signature.
    fn as_signature(&self) -> Result<(ECDSASignature, RecoveryId), SignatureError> {
        let recovery_id = self.recovery_id()?;
        let signature = {
            let r_bytes = self.r.to_be_bytes::<32>();
            let s_bytes = self.s.to_be_bytes::<32>();

            let mut bytes = [0u8; 64];
            bytes[..32].copy_from_slice(&r_bytes);
            bytes[32..64].copy_from_slice(&s_bytes);
            ECDSASignature::from_slice(&bytes).map_err(SignatureError::ECDSAError)?
        };

        Ok((signature, recovery_id))
    }

    /// Retrieve the recovery ID.
    pub fn recovery_id(&self) -> Result<RecoveryId, SignatureError> {
        let standard_v = normalize_recovery_id(self.v);
        RecoveryId::try_from(standard_v).map_err(SignatureError::ECDSAError)
    }

    /// Copies and serializes `self` into a new `Vec` with the recovery id
    /// included
    #[allow(clippy::wrong_self_convention)]
    pub fn to_vec(&self) -> Vec<u8> {
        self.into()
    }
}

// We need a custom implementation to avoid the struct being treated as an RLP
// list.
impl alloy_rlp::Decodable for SignatureWithRecoveryId {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let decoded = Self {
            // The order of these fields determines decoding order.
            v: u64::decode(buf)?,
            r: U256::decode(buf)?,
            s: U256::decode(buf)?,
        };

        Ok(decoded)
    }
}

// We need a custom implementation to avoid the struct being treated as an RLP
// list.
impl alloy_rlp::Encodable for SignatureWithRecoveryId {
    fn encode(&self, out: &mut dyn BufMut) {
        // The order of these fields determines decoding order.
        self.v.encode(out);
        self.r.encode(out);
        self.s.encode(out);
    }

    fn length(&self) -> usize {
        self.r.length() + self.s.length() + self.v.length()
    }
}

impl Recoverable for SignatureWithRecoveryId {
    fn recover_address(&self, message: RecoveryMessage) -> Result<Address, SignatureError> {
        self.recover(message)
    }
}

impl Signature for SignatureWithRecoveryId {
    fn r(&self) -> U256 {
        self.r
    }

    fn s(&self) -> U256 {
        self.s
    }

    fn v(&self) -> u64 {
        self.v
    }

    fn y_parity(&self) -> Option<bool> {
        None
    }
}

fn normalize_recovery_id(v: u64) -> u8 {
    match v {
        0 | 27 => 0,
        1 | 28 => 1,
        v if v >= 35 => ((v - 1) % 2) as _,
        _ => 4,
    }
}

impl<'a> TryFrom<&'a [u8]> for SignatureWithRecoveryId {
    type Error = SignatureError;

    /// Parses a raw signature which is expected to be 65 bytes long where
    /// the first 32 bytes is the `r` value, the second 32 bytes the `s` value
    /// and the final byte is the `v` value in 'Electrum' notation.
    fn try_from(bytes: &'a [u8]) -> Result<Self, Self::Error> {
        if bytes.len() != 65 {
            return Err(SignatureError::InvalidLength(bytes.len()));
        }

        let (r_bytes, remainder) = bytes.split_at(32);
        let r = U256::from_be_bytes::<32>(r_bytes.try_into().unwrap());

        let (s_bytes, remainder) = remainder.split_at(32);
        let s = U256::from_be_bytes::<32>(s_bytes.try_into().unwrap());

        let v = remainder[0];

        Ok(SignatureWithRecoveryId { r, s, v: v.into() })
    }
}

#[cfg(feature = "std")]
impl FromStr for SignatureWithRecoveryId {
    type Err = SignatureError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.strip_prefix("0x").unwrap_or(s);
        let bytes = hex::decode(s).map_err(SignatureError::DecodingError)?;
        SignatureWithRecoveryId::try_from(&bytes[..])
    }
}

impl From<&SignatureWithRecoveryId> for [u8; 65] {
    fn from(src: &SignatureWithRecoveryId) -> [u8; 65] {
        let mut sig = [0u8; 65];
        let r_bytes = src.r.to_be_bytes::<32>();
        let s_bytes = src.s.to_be_bytes::<32>();
        sig[..32].copy_from_slice(&r_bytes);
        sig[32..64].copy_from_slice(&s_bytes);
        // TODO: What if we try to serialize a signature where
        // the `v` is not normalized?

        // The u64 to u8 cast is safe because `sig.v` can only ever be 27 or 28
        // here. Regarding EIP-155, the modification to `v` happens during tx
        // creation only _after_ the transaction is signed using
        // `ethers_signers::to_eip155_v`.
        sig[64] = src.v as u8;
        sig
    }
}

impl From<SignatureWithRecoveryId> for [u8; 65] {
    fn from(src: SignatureWithRecoveryId) -> [u8; 65] {
        <[u8; 65]>::from(&src)
    }
}

impl From<&SignatureWithRecoveryId> for Vec<u8> {
    fn from(src: &SignatureWithRecoveryId) -> Vec<u8> {
        <[u8; 65]>::from(src).to_vec()
    }
}

impl From<SignatureWithRecoveryId> for Vec<u8> {
    fn from(src: SignatureWithRecoveryId) -> Vec<u8> {
        <[u8; 65]>::from(&src).to_vec()
    }
}

impl From<&SignatureWithRecoveryId> for Bytes {
    fn from(src: &SignatureWithRecoveryId) -> Self {
        Bytes::from(Vec::<u8>::from(src))
    }
}

impl From<&[u8]> for RecoveryMessage {
    fn from(s: &[u8]) -> Self {
        s.to_owned().into()
    }
}

impl From<Vec<u8>> for RecoveryMessage {
    fn from(s: Vec<u8>) -> Self {
        RecoveryMessage::Data(s)
    }
}

impl From<&str> for RecoveryMessage {
    fn from(s: &str) -> Self {
        s.as_bytes().to_owned().into()
    }
}

impl From<String> for RecoveryMessage {
    fn from(s: String) -> Self {
        RecoveryMessage::Data(s.into_bytes())
    }
}

impl From<[u8; 32]> for RecoveryMessage {
    fn from(hash: [u8; 32]) -> Self {
        B256::from(hash).into()
    }
}

impl From<B256> for RecoveryMessage {
    fn from(hash: B256) -> Self {
        RecoveryMessage::Hash(hash)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use edr_test_utils::secret_key::{
        secret_key_from_str, secret_key_to_address, secret_key_to_str,
    };

    use super::*;

    #[test]
    fn recover_web3_signature() {
        // test vector taken from:
        // https://web3js.readthedocs.io/en/v1.2.2/web3-eth-accounts.html#sign
        let signature = SignatureWithRecoveryId::from_str(
            "0xb91467e570a6466aa9e9876cbcd013baba02900b8979d43fe208a4a4f339f5fd6007e74cd82e037b800186422fc2da167c747ef045e5d18a5f5d4300f8e1a0291c"
        ).expect("could not parse signature");
        assert_eq!(
            signature.recover("Some data").unwrap(),
            Address::from_str("0x2c7536E3605D9C16a7a3D7b1898e529396a65c23").unwrap()
        );
    }

    #[test]
    fn signature_from_str() {
        let s1 = SignatureWithRecoveryId::from_str(
            "0xaa231fbe0ed2b5418e6ba7c19bee2522852955ec50996c02a2fe3e71d30ddaf1645baf4823fea7cb4fcc7150842493847cfb6a6d63ab93e8ee928ee3f61f503500"
        ).expect("could not parse 0x-prefixed signature");

        let s2 = SignatureWithRecoveryId::from_str(
            "aa231fbe0ed2b5418e6ba7c19bee2522852955ec50996c02a2fe3e71d30ddaf1645baf4823fea7cb4fcc7150842493847cfb6a6d63ab93e8ee928ee3f61f503500"
        ).expect("could not parse non-prefixed signature");

        assert_eq!(s1, s2);
    }

    #[test]
    fn test_secret_key_to_address() {
        // `hardhat node`s default addresses are shown on startup. this is the first
        // one:     Account #0: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
        // (10000 ETH)     Secret Key:
        // 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
        // we'll use these as fixtures.

        let expected_address = Address::from_str("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")
            .expect("should parse address from string");

        let actual_address = secret_key_to_address(
            "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        )
        .expect("should derive address");
        assert_eq!(actual_address, expected_address);
    }

    #[test]
    fn test_signature_new() {
        fn verify<MsgOrHash>(msg_input: MsgOrHash, hashed_message: B256)
        where
            MsgOrHash: Into<RecoveryMessage>,
        {
            let secret_key_str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
            let secret_key = secret_key_from_str(secret_key_str).unwrap();

            let signature = SignatureWithRecoveryId::new(msg_input, &secret_key).unwrap();

            let recovered_address = signature.recover(hashed_message).unwrap();

            assert_eq!(
                recovered_address,
                secret_key_to_address(secret_key_str).unwrap()
            );
        }

        let message = "whatever";
        let hashed_message = hash_message(message);

        verify(message, hashed_message);
        verify(hashed_message, hashed_message);
    }

    #[test]
    fn test_from_str_to_str_secret_key() {
        let secret_key_str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let secret_key = secret_key_from_str(secret_key_str).unwrap();
        let secret_key_str_result = secret_key_to_str(&secret_key);
        assert_eq!(secret_key_str, secret_key_str_result);
    }
}
