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
pub struct Eip1559 {
    // The order of these fields determines encoding order.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde::u64"))]
    pub chain_id: u64,
    #[cfg_attr(feature = "serde", serde(with = "crate::serde::u64"))]
    pub nonce: u64,
    pub max_priority_fee_per_gas: U256,
    pub max_fee_per_gas: U256,
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

impl Eip1559 {
    /// Returns the caller/signer of the transaction.
    pub fn caller(&self) -> &Address {
        self.signature.caller()
    }

    pub fn hash(&self) -> &B256 {
        self.hash.get_or_init(|| {
            let encoded = alloy_rlp::encode(self);
            let enveloped = envelop_bytes(2, &encoded);

            keccak256(enveloped)
        })
    }
}

impl From<Eip1559> for TxEnv {
    fn from(value: Eip1559) -> Self {
        TxEnv {
            caller: *value.caller(),
            gas_limit: value.gas_limit,
            gas_price: value.max_fee_per_gas,
            transact_to: kind_to_transact_to(value.kind),
            value: value.value,
            data: value.input,
            nonce: Some(value.nonce),
            chain_id: Some(value.chain_id),
            access_list: value.access_list.into(),
            gas_priority_fee: Some(value.max_priority_fee_per_gas),
            blob_hashes: Vec::new(),
            max_fee_per_blob_gas: None,
            authorization_list: None,
        }
    }
}

impl PartialEq for Eip1559 {
    fn eq(&self, other: &Self) -> bool {
        self.chain_id == other.chain_id
            && self.nonce == other.nonce
            && self.max_priority_fee_per_gas == other.max_priority_fee_per_gas
            && self.max_fee_per_gas == other.max_fee_per_gas
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
    pub max_priority_fee_per_gas: U256,
    pub max_fee_per_gas: U256,
    pub gas_limit: u64,
    pub kind: TxKind,
    pub value: U256,
    pub input: Bytes,
    pub access_list: AccessList,
    pub signature: signature::SignatureWithYParity,
}

impl alloy_rlp::Decodable for Eip1559 {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let transaction = Decodable::decode(buf)?;
        let request = transaction::request::Eip1559::from(&transaction);

        let signature = Fakeable::recover(transaction.signature, request.hash().into())
            .map_err(|_error| alloy_rlp::Error::Custom("Invalid Signature"))?;

        Ok(Self {
            chain_id: transaction.chain_id,
            nonce: transaction.nonce,
            max_priority_fee_per_gas: transaction.max_priority_fee_per_gas,
            max_fee_per_gas: transaction.max_fee_per_gas,
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

impl From<&Decodable> for transaction::request::Eip1559 {
    fn from(value: &Decodable) -> Self {
        Self {
            chain_id: value.chain_id,
            nonce: value.nonce,
            max_priority_fee_per_gas: value.max_priority_fee_per_gas,
            max_fee_per_gas: value.max_fee_per_gas,
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
    use crate::{
        signature::{secret_key_from_str, secret_key_to_address},
        AccessListItem,
    };

    const DUMMY_SECRET_KEY: &str =
        "e331b6d69882b4cb4ea581d88e0b604039a3de5967688d3dcffdd2270c0fd109";

    fn dummy_request() -> transaction::request::Eip1559 {
        let to = Address::from_str("0xc014ba5ec014ba5ec014ba5ec014ba5ec014ba5e").unwrap();
        let input = hex::decode("1234").unwrap();
        transaction::request::Eip1559 {
            chain_id: 1,
            nonce: 1,
            max_priority_fee_per_gas: U256::from(2),
            max_fee_per_gas: U256::from(5),
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
        secret_key_from_str(DUMMY_SECRET_KEY).unwrap()
    }

    #[test]
    fn test_eip1559_signed_transaction_encoding() {
        // Generated by Hardhat
        let expected =
            hex::decode("f8be010102050394c014ba5ec014ba5ec014ba5ec014ba5ec014ba5e04821234f85bf859940000000000000000000000000000000000000000f842a00000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000000000000000000000000000000000000000000101a07764e376b5b4090264f73abee68ebb5fdc9f76050eff800237e5a2bedadcd7eda044c0ae9b07c75cf4e0a14aebfe792ab2fdccd7d89550b166b1b4a4ece0054f02")
                .unwrap();

        let request = dummy_request();
        let signed = request.sign(&dummy_secret_key()).unwrap();

        let encoded = alloy_rlp::encode(&signed);
        assert_eq!(expected, encoded);
    }

    #[test]
    fn test_eip1559_signed_transaction_hash() {
        // Generated by hardhat
        let expected = B256::from_slice(
            &hex::decode("043d6f6de2e81af3f48d6c64d4cdfc7576f8754c73569bc6903e50f3c92988d8")
                .unwrap(),
        );

        let request = dummy_request();
        let signed = request.sign(&dummy_secret_key()).unwrap();

        assert_eq!(expected, *signed.hash());
    }

    #[test]
    fn test_eip1559_signed_transaction_caller() {
        let request = dummy_request();
        let signed = request.sign(&dummy_secret_key()).unwrap();

        let expected = secret_key_to_address(DUMMY_SECRET_KEY)
            .expect("Failed to retrieve address from secret key");

        assert_eq!(expected, *signed.caller());
    }

    #[test]
    fn test_eip1559_signed_transaction_rlp() {
        let request = dummy_request();
        let signed = request.sign(&dummy_secret_key()).unwrap();

        let encoded = alloy_rlp::encode(&signed);
        assert_eq!(signed, Eip1559::decode(&mut encoded.as_slice()).unwrap());
    }
}
