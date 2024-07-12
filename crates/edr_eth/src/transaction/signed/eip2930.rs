use std::sync::OnceLock;

use alloy_rlp::{RlpDecodable, RlpEncodable};
use revm_primitives::{keccak256, TxEnv};

use super::kind_to_transact_to;
use crate::{
    signature::{self, Fakeable},
    transaction::{self, TxKind},
    utils::envelop_bytes,
    AccessList, Address, Bytes, B256, U256,
};

#[derive(Clone, Debug, Eq, RlpEncodable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Eip2930 {
    // The order of these fields determines encoding order.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde::u64"))]
    pub chain_id: u64,
    #[cfg_attr(feature = "serde", serde(with = "crate::serde::u64"))]
    pub nonce: u64,
    pub gas_price: U256,
    #[cfg_attr(feature = "serde", serde(with = "crate::serde::u64"))]
    pub gas_limit: u64,
    pub kind: TxKind,
    pub value: U256,
    pub input: Bytes,
    pub access_list: AccessList,
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub signature: signature::Fakeable<signature::SignatureWithYParity>,
    /// Cached transaction hash
    #[rlp(default)]
    #[rlp(skip)]
    #[cfg_attr(feature = "serde", serde(skip))]
    pub hash: OnceLock<B256>,
}

impl Eip2930 {
    /// Returns the caller/signer of the transaction.
    pub fn caller(&self) -> &Address {
        self.signature.caller()
    }

    pub fn hash(&self) -> &B256 {
        self.hash.get_or_init(|| {
            let encoded = alloy_rlp::encode(self);
            let enveloped = envelop_bytes(1, &encoded);

            keccak256(enveloped)
        })
    }
}

impl From<Eip2930> for TxEnv {
    fn from(value: Eip2930) -> Self {
        TxEnv {
            caller: *value.caller(),
            gas_limit: value.gas_limit,
            gas_price: value.gas_price,
            transact_to: kind_to_transact_to(value.kind),
            value: value.value,
            data: value.input,
            nonce: Some(value.nonce),
            chain_id: Some(value.chain_id),
            access_list: value.access_list.into(),
            gas_priority_fee: None,
            blob_hashes: Vec::new(),
            max_fee_per_blob_gas: None,
            authorization_list: None,
        }
    }
}

impl PartialEq for Eip2930 {
    fn eq(&self, other: &Self) -> bool {
        self.chain_id == other.chain_id
            && self.nonce == other.nonce
            && self.gas_price == other.gas_price
            && self.gas_limit == other.gas_limit
            && self.kind == other.kind
            && self.value == other.value
            && self.input == other.input
            && self.access_list == other.access_list
            && self.signature == other.signature
    }
}

#[derive(RlpDecodable)]
struct Decodable {
    // The order of these fields determines decoding order.
    pub chain_id: u64,
    pub nonce: u64,
    pub gas_price: U256,
    pub gas_limit: u64,
    pub kind: TxKind,
    pub value: U256,
    pub input: Bytes,
    pub access_list: AccessList,
    pub signature: signature::SignatureWithYParity,
}

impl alloy_rlp::Decodable for Eip2930 {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let transaction = Decodable::decode(buf)?;
        let request = transaction::request::Eip2930::from(&transaction);

        let signature = Fakeable::recover(transaction.signature, request.hash().into())
            .map_err(|_error| alloy_rlp::Error::Custom("Invalid Signature"))?;

        Ok(Self {
            chain_id: transaction.chain_id,
            nonce: transaction.nonce,
            gas_price: transaction.gas_price,
            gas_limit: transaction.gas_limit,
            kind: transaction.kind,
            value: transaction.value,
            input: transaction.input,
            access_list: transaction.access_list,
            signature,
            hash: OnceLock::new(),
        })
    }
}

impl From<&Decodable> for transaction::request::Eip2930 {
    fn from(value: &Decodable) -> Self {
        Self {
            chain_id: value.chain_id,
            nonce: value.nonce,
            gas_price: value.gas_price,
            gas_limit: value.gas_limit,
            kind: value.kind,
            value: value.value,
            input: value.input.clone(),
            access_list: value.access_list.0.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_rlp::Decodable;
    use k256::SecretKey;

    use super::*;
    use crate::{signature::secret_key_from_str, AccessListItem};

    fn dummy_request() -> transaction::request::Eip2930 {
        let to = Address::from_str("0xc014ba5ec014ba5ec014ba5ec014ba5ec014ba5e").unwrap();
        let input = hex::decode("1234").unwrap();
        transaction::request::Eip2930 {
            chain_id: 1,
            nonce: 1,
            gas_price: U256::from(2),
            gas_limit: 3,
            kind: TxKind::Call(to),
            value: U256::from(4),
            input: Bytes::from(input),
            access_list: vec![AccessListItem {
                address: Address::ZERO,
                storage_keys: vec![B256::ZERO, B256::from(U256::from(1))],
            }],
        }
    }

    fn dummy_secret_key() -> SecretKey {
        secret_key_from_str("e331b6d69882b4cb4ea581d88e0b604039a3de5967688d3dcffdd2270c0fd109")
            .unwrap()
    }

    #[test]
    fn test_eip2930_signed_transaction_encoding() {
        // Generated by Hardhat
        let expected =
            hex::decode("f8bd0101020394c014ba5ec014ba5ec014ba5ec014ba5ec014ba5e04821234f85bf859940000000000000000000000000000000000000000f842a00000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000000000000000000000000000000000000000000101a0a9f9f0c845cc2d257838df2679a59af6f19055012ce1de11ba25b4ca9df503cfa02c70c54cf6c49b4a641b269c93308fa07de541aa3bcd3fce0fc722aaabe3a8d8")
                .unwrap();

        let request = dummy_request();
        let signed = request.sign(&dummy_secret_key()).unwrap();

        let encoded = alloy_rlp::encode(&signed);
        assert_eq!(expected, encoded);
    }

    #[test]
    fn test_eip2930_signed_transaction_hash() {
        // Generated by hardhat
        let expected = B256::from_slice(
            &hex::decode("1d4f5ef5c7b4b0bd61d4dd622615ec280ae5b9a57136ce6b7686025999220611")
                .unwrap(),
        );

        let request = dummy_request();
        let signed = request.sign(&dummy_secret_key()).unwrap();

        assert_eq!(expected, *signed.hash());
    }

    #[test]
    fn test_eip2930_signed_transaction_rlp() {
        let request = dummy_request();
        let signed = request.sign(&dummy_secret_key()).unwrap();

        let encoded = alloy_rlp::encode(&signed);
        assert_eq!(signed, Eip2930::decode(&mut encoded.as_slice()).unwrap());
    }
}
