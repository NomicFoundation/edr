use std::sync::OnceLock;

use alloy_rlp::{Encodable as _, RlpDecodable, RlpEncodable};
use edr_evm_spec::ExecutableTransaction;
use edr_primitives::{Address, Bytes, B256, U256};
use edr_signer::{FakeableSignature, SignatureWithYParity};
use revm_primitives::{keccak256, TxKind};

use crate::{request, utils::enveloped};

#[derive(Clone, Debug, Eq, serde::Serialize, RlpEncodable)]
pub struct Eip2930 {
    // The order of these fields determines encoding order.
    #[serde(with = "alloy_serde::quantity")]
    pub chain_id: u64,
    #[serde(with = "alloy_serde::quantity")]
    pub nonce: u64,
    #[serde(with = "alloy_serde::quantity")]
    pub gas_price: u128,
    #[serde(with = "alloy_serde::quantity")]
    pub gas_limit: u64,
    pub kind: TxKind,
    pub value: U256,
    pub input: Bytes,
    pub access_list: edr_eip2930::AccessList,
    #[serde(flatten)]
    pub signature: FakeableSignature<SignatureWithYParity>,
    /// Cached transaction hash
    #[rlp(default)]
    #[rlp(skip)]
    #[serde(skip)]
    pub hash: OnceLock<B256>,
    /// Cached RLP-encoding
    #[rlp(skip)]
    #[serde(skip)]
    pub rlp_encoding: OnceLock<Bytes>,
}

impl Eip2930 {
    /// The type identifier for an EIP-2930 transaction.
    pub const TYPE: u8 = request::Eip2930::TYPE;
}

impl ExecutableTransaction for Eip2930 {
    fn caller(&self) -> &Address {
        self.signature.caller()
    }

    fn gas_limit(&self) -> u64 {
        self.gas_limit
    }

    fn gas_price(&self) -> &u128 {
        &self.gas_price
    }

    fn kind(&self) -> TxKind {
        self.kind
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

    fn access_list(&self) -> Option<&[edr_eip2930::AccessListItem]> {
        Some(&self.access_list)
    }

    fn effective_gas_price(&self, _block_base_fee: u128) -> Option<u128> {
        None
    }

    fn max_fee_per_gas(&self) -> Option<&u128> {
        None
    }

    fn max_priority_fee_per_gas(&self) -> Option<&u128> {
        None
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

    fn authorization_list(&self) -> Option<&[edr_eip7702::SignedAuthorization]> {
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
    pub gas_price: u128,
    pub gas_limit: u64,
    pub kind: TxKind,
    pub value: U256,
    pub input: Bytes,
    pub access_list: edr_eip2930::AccessList,
    pub signature: SignatureWithYParity,
}

impl alloy_rlp::Decodable for Eip2930 {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let transaction = Decodable::decode(buf)?;
        let request = request::Eip2930::from(&transaction);

        let signature = FakeableSignature::recover(transaction.signature, request.hash().into())
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
            rlp_encoding: OnceLock::new(),
        })
    }
}

impl From<&Decodable> for request::Eip2930 {
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
    use edr_test_utils::secret_key::secret_key_from_str;
    use k256::SecretKey;

    use super::*;

    fn dummy_request() -> request::Eip2930 {
        let to = Address::from_str("0xc014ba5ec014ba5ec014ba5ec014ba5ec014ba5e").unwrap();
        let input = hex::decode("1234").unwrap();
        request::Eip2930 {
            chain_id: 1,
            nonce: 1,
            gas_price: 2,
            gas_limit: 3,
            kind: TxKind::Call(to),
            value: U256::from(4),
            input: Bytes::from(input),
            access_list: vec![edr_eip2930::AccessListItem {
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

        assert_eq!(expected, *signed.transaction_hash());
    }

    #[test]
    fn test_eip2930_signed_transaction_rlp() {
        let request = dummy_request();
        let signed = request.sign(&dummy_secret_key()).unwrap();

        let encoded = alloy_rlp::encode(&signed);
        assert_eq!(signed, Eip2930::decode(&mut encoded.as_slice()).unwrap());
    }
}
