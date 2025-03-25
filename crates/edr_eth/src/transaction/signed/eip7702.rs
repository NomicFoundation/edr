use std::sync::OnceLock;

use alloy_rlp::{Encodable as _, RlpDecodable, RlpEncodable};

use crate::{
    eips::{eip2930, eip7702},
    keccak256, signature,
    transaction::{self, request, ComputeTransactionHash as _, ExecutableTransaction, TxKind},
    utils::enveloped,
    Address, Bytes, B256, U256,
};

#[derive(Clone, Debug, Eq, RlpEncodable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Eip7702 {
    // The order of these fields determines encoding order.
    #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
    pub chain_id: u64,
    #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
    pub nonce: u64,
    #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
    pub max_priority_fee_per_gas: u128,
    #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
    pub max_fee_per_gas: u128,
    #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
    pub gas_limit: u64,
    pub to: Address,
    pub value: U256,
    pub input: Bytes,
    pub access_list: eip2930::AccessList,
    pub authorization_list: Vec<eip7702::SignedAuthorization>,
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub signature: signature::Fakeable<signature::SignatureWithYParity>,
    /// Cached transaction hash
    #[rlp(skip)]
    #[cfg_attr(feature = "serde", serde(skip))]
    pub hash: OnceLock<B256>,
    /// Cached RLP-encoding
    #[rlp(skip)]
    #[cfg_attr(feature = "serde", serde(skip))]
    pub rlp_encoding: OnceLock<Bytes>,
}

impl Eip7702 {
    pub const TYPE: u8 = request::Eip7702::TYPE;
}

impl ExecutableTransaction for Eip7702 {
    fn caller(&self) -> &Address {
        self.signature.caller()
    }

    fn gas_limit(&self) -> u64 {
        self.gas_limit
    }

    fn gas_price(&self) -> &u128 {
        &self.max_fee_per_gas
    }

    fn kind(&self) -> TxKind {
        TxKind::Call(self.to)
    }

    fn value(&self) -> &U256 {
        &self.value
    }

    fn data(&self) -> &Bytes {
        &self.input
    }

    fn nonce(&self) -> u64 {
        self.nonce
    }

    fn chain_id(&self) -> Option<u64> {
        Some(self.chain_id)
    }

    fn access_list(&self) -> Option<&[eip2930::AccessListItem]> {
        Some(&self.access_list)
    }

    fn effective_gas_price(&self, block_base_fee: u128) -> Option<u128> {
        Some(
            self.max_fee_per_gas
                .min(block_base_fee + self.max_priority_fee_per_gas),
        )
    }

    fn max_fee_per_gas(&self) -> Option<&u128> {
        Some(&self.max_fee_per_gas)
    }

    fn max_priority_fee_per_gas(&self) -> Option<&u128> {
        Some(&self.max_priority_fee_per_gas)
    }

    fn blob_hashes(&self) -> &[B256] {
        &[]
    }

    fn max_fee_per_blob_gas(&self) -> Option<&u128> {
        None
    }

    fn total_blob_gas(&self) -> Option<u64> {
        None
    }

    fn authorization_list(&self) -> Option<&[eip7702::SignedAuthorization]> {
        None
    }

    fn rlp_encoding(&self) -> &Bytes {
        self.rlp_encoding.get_or_init(|| {
            let mut encoded = Vec::with_capacity(1 + self.length());
            enveloped(Self::TYPE, self, &mut encoded);
            encoded.into()
        })
    }

    fn transaction_hash(&self) -> &B256 {
        self.hash.get_or_init(|| keccak256(self.rlp_encoding()))
    }
}

impl PartialEq for Eip7702 {
    fn eq(&self, other: &Self) -> bool {
        self.chain_id == other.chain_id
            && self.nonce == other.nonce
            && self.max_priority_fee_per_gas == other.max_priority_fee_per_gas
            && self.max_fee_per_gas == other.max_fee_per_gas
            && self.gas_limit == other.gas_limit
            && self.to == other.to
            && self.value == other.value
            && self.input == other.input
            && self.access_list == other.access_list
            && self.authorization_list == other.authorization_list
            && self.signature == other.signature
    }
}

#[derive(RlpDecodable)]
struct Decodable {
    // The order of these fields determines decoding order.
    pub chain_id: u64,
    pub nonce: u64,
    pub max_priority_fee_per_gas: u128,
    pub max_fee_per_gas: u128,
    pub gas_limit: u64,
    pub to: Address,
    pub value: U256,
    pub input: Bytes,
    pub access_list: eip2930::AccessList,
    pub authorization_list: Vec<eip7702::SignedAuthorization>,
    pub signature: signature::SignatureWithYParity,
}

impl alloy_rlp::Decodable for Eip7702 {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let transaction = Decodable::decode(buf)?;
        let request = transaction::request::Eip7702::from(&transaction);

        let signature = signature::Fakeable::recover(
            transaction.signature,
            request.compute_transaction_hash().into(),
        )
        .map_err(|_error| alloy_rlp::Error::Custom("Invalid Signature"))?;

        Ok(Self {
            chain_id: transaction.chain_id,
            nonce: transaction.nonce,
            max_priority_fee_per_gas: transaction.max_priority_fee_per_gas,
            max_fee_per_gas: transaction.max_fee_per_gas,
            gas_limit: transaction.gas_limit,
            to: transaction.to,
            value: transaction.value,
            input: transaction.input,
            access_list: transaction.access_list,
            authorization_list: transaction.authorization_list,
            signature,
            hash: OnceLock::new(),
            rlp_encoding: OnceLock::new(),
        })
    }
}

impl From<&Decodable> for transaction::request::Eip7702 {
    fn from(value: &Decodable) -> Self {
        Self {
            chain_id: value.chain_id,
            nonce: value.nonce,
            max_priority_fee_per_gas: value.max_priority_fee_per_gas,
            max_fee_per_gas: value.max_fee_per_gas,
            gas_limit: value.gas_limit,
            to: value.to,
            value: value.value,
            input: value.input.clone(),
            access_list: value.access_list.0.clone(),
            authorization_list: value.authorization_list.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    mod expectation {
        use core::str::FromStr as _;

        use edr_defaults::SECRET_KEYS;
        use edr_test_utils::secret_key::{secret_key_from_str, SecretKey, SignatureError};
        use hex::FromHexError;

        use super::*;

        const CHAIN_ID: u64 = 0x7a69;

        pub const TRANSACTION_HASH: B256 =
            b256!("235bb5a9856798eee27ec065a3aef0dc294a02713fce10c79321e436c98e1aab");

        pub fn signed_authorization() -> eip7702::SignedAuthorization {
            eip7702::SignedAuthorization::new_unchecked(
                eip7702::Authorization {
                    chain_id: U256::from(CHAIN_ID),
                    address: address!("0x1234567890123456789012345678901234567890"),
                    nonce: 1,
                },
                1,
                U256::from_str(
                    "0xeb775e0a2b7a15ea4938921e1ab255c84270e25c2c384b2adc32c73cd70273d6",
                )
                .expect("R value is valid"),
                U256::from_str(
                    "0x46b9bec1961318a644db6cd9c7fc4e8d7c6f40d9165fc8958f3aff2216ed6f7c",
                )
                .expect("S value is valid"),
            )
        }

        pub fn raw() -> Result<Vec<u8>, FromHexError> {
            hex::decode("04f8cc827a6980843b9aca00848321560082f61894f39fd6e51aad88f6f4ce6ab8827279cfffb922668080c0f85ef85c827a699412345678901234567890123456789012345678900101a0eb775e0a2b7a15ea4938921e1ab255c84270e25c2c384b2adc32c73cd70273d6a046b9bec1961318a644db6cd9c7fc4e8d7c6f40d9165fc8958f3aff2216ed6f7c01a0be47a039954e4dfb7f08927ef7f072e0ec7510290e3c4c1405f3bf0329d0be51a06f291c455321a863d4c8ebbd73d58e809328918bcb5555958247ca6ec27feec8")
        }

        // Test vector generated using secret key in `dummy_secret_key`.
        pub fn request() -> anyhow::Result<transaction::request::Eip7702> {
            let request = transaction::request::Eip7702 {
                chain_id: CHAIN_ID,
                nonce: 0,
                max_priority_fee_per_gas: 1_000_000_000,
                max_fee_per_gas: 2_200_000_000,
                gas_limit: 63_000,
                to: address!("0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"),
                value: U256::ZERO,
                input: Bytes::new(),
                access_list: Vec::new(),
                authorization_list: vec![signed_authorization()],
            };
            Ok(request)
        }

        pub fn signed() -> anyhow::Result<transaction::Signed> {
            let request = expectation::request()?;

            let secret_key = expectation::secret_key()?;
            let signed = request.sign(&secret_key)?;

            Ok(signed.into())
        }

        pub fn secret_key() -> Result<SecretKey, SignatureError> {
            secret_key_from_str(SECRET_KEYS[0])
        }
    }

    use alloy_rlp::Decodable as _;

    use super::*;
    use crate::{address, b256, signature::public_key_to_address};

    #[test]
    fn decoding() -> anyhow::Result<()> {
        let raw_transaction = expectation::raw()?;

        let decoded = transaction::Signed::decode(&mut raw_transaction.as_slice())?;
        let expected = expectation::signed()?;
        assert_eq!(decoded, expected);

        Ok(())
    }

    #[test]
    fn encoding() -> anyhow::Result<()> {
        let signed = expectation::signed()?;

        let encoded = alloy_rlp::encode(&signed);
        let expected = expectation::raw()?;
        assert_eq!(encoded, expected);

        Ok(())
    }

    #[test]
    fn transaction_hash() -> anyhow::Result<()> {
        let signed = expectation::signed()?;

        let transaction_hash = signed.transaction_hash();
        assert_eq!(*transaction_hash, expectation::TRANSACTION_HASH);

        Ok(())
    }

    #[test]
    fn recover_authority() -> anyhow::Result<()> {
        let authorization = expectation::signed_authorization();

        let secret_key = expectation::secret_key()?;

        let expected = public_key_to_address(secret_key.public_key());
        let recovered = authorization.recover_authority()?;
        assert_eq!(recovered, expected);

        Ok(())
    }

    #[test]
    fn recover_caller() -> anyhow::Result<()> {
        let signed = expectation::signed()?;

        let secret_key = expectation::secret_key()?;

        let expected = public_key_to_address(secret_key.public_key());
        let recovered = signed.caller();
        assert_eq!(recovered, &expected);

        Ok(())
    }
}
