use std::sync::OnceLock;

use alloy_rlp::RlpEncodable;

use crate::{
    eips::{eip2930, eip7702},
    keccak256,
    signature::{self, Signature},
    transaction::{self, ExecutableTransaction, TxKind},
    Address, Bytes, B256, U256,
};

#[derive(Clone, Debug, Eq, RlpEncodable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Eip155 {
    // The order of these fields determines encoding order.
    #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
    pub nonce: u64,
    #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
    pub gas_price: u128,
    #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
    pub gas_limit: u64,
    pub kind: TxKind,
    pub value: U256,
    pub input: Bytes,
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub signature: signature::Fakeable<signature::SignatureWithRecoveryId>,
    /// Cached transaction hash
    #[rlp(default)]
    #[rlp(skip)]
    #[cfg_attr(feature = "serde", serde(skip))]
    pub hash: OnceLock<B256>,
    /// Cached RLP-encoding
    #[rlp(skip)]
    #[cfg_attr(feature = "serde", serde(skip))]
    pub rlp_encoding: OnceLock<Bytes>,
}

impl Eip155 {
    /// The type identifier for a post-EIP-155 transaction.
    pub const TYPE: u8 = transaction::request::Eip155::TYPE;
}

impl ExecutableTransaction for Eip155 {
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
        Some(v_to_chain_id(self.signature.v()))
    }

    fn access_list(&self) -> Option<&[eip2930::AccessListItem]> {
        None
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

    fn authorization_list(&self) -> Option<&[eip7702::SignedAuthorization]> {
        None
    }

    fn rlp_encoding(&self) -> &Bytes {
        self.rlp_encoding
            .get_or_init(|| alloy_rlp::encode(self).into())
    }

    fn transaction_hash(&self) -> &B256 {
        self.hash.get_or_init(|| keccak256(alloy_rlp::encode(self)))
    }
}

impl From<transaction::signed::Legacy> for Eip155 {
    fn from(tx: transaction::signed::Legacy) -> Self {
        Self {
            nonce: tx.nonce,
            gas_price: tx.gas_price,
            gas_limit: tx.gas_limit,
            kind: tx.kind,
            value: tx.value,
            input: tx.input,
            signature: tx.signature,
            hash: tx.hash,
            rlp_encoding: tx.rlp_encoding,
        }
    }
}

impl PartialEq for Eip155 {
    fn eq(&self, other: &Self) -> bool {
        self.nonce == other.nonce
            && self.gas_price == other.gas_price
            && self.gas_limit == other.gas_limit
            && self.kind == other.kind
            && self.value == other.value
            && self.input == other.input
            && self.signature == other.signature
    }
}

/// Converts a V-value to a chain ID.
pub(super) fn v_to_chain_id(v: u64) -> u64 {
    (v - 35) / 2
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_rlp::Decodable as _;
    use edr_test_utils::secret_key::secret_key_from_str;
    use k256::SecretKey;

    use super::*;
    use crate::transaction::signed::PreOrPostEip155;

    fn dummy_request() -> transaction::request::Eip155 {
        let to = Address::from_str("0xc014ba5ec014ba5ec014ba5ec014ba5ec014ba5e").unwrap();
        let input = hex::decode("1234").unwrap();
        transaction::request::Eip155 {
            nonce: 1,
            gas_price: 2,
            gas_limit: 3,
            kind: TxKind::Call(to),
            value: U256::from(4),
            input: Bytes::from(input),
            chain_id: 1,
        }
    }

    fn dummy_secret_key() -> SecretKey {
        secret_key_from_str("e331b6d69882b4cb4ea581d88e0b604039a3de5967688d3dcffdd2270c0fd109")
            .unwrap()
    }

    #[test]
    fn test_eip155_signed_transaction_encoding() {
        // Generated by Hardhat
        let expected =
            hex::decode("f85f01020394c014ba5ec014ba5ec014ba5ec014ba5ec014ba5e0482123426a0fc9f82c3002f9ed8c05d6e8e821cf14eab65a1b4647e002957e170149393f40ba077f230fafdb096cf80762af3d3f4243f02e754f363fb9443d914c3a286fa2774")
                .unwrap();

        let request = dummy_request();
        let signed = request.sign(&dummy_secret_key()).unwrap();

        let encoded = alloy_rlp::encode(&signed);
        assert_eq!(expected, encoded);
    }

    #[test]
    fn test_eip155_signed_transaction_hash() {
        // Generated by hardhat
        let expected = B256::from_slice(
            &hex::decode("4da115513cdabaed0e9c9e503acd2fa7af29e5baae7f79a6ffa878b3ff380de6")
                .unwrap(),
        );

        let request = dummy_request();
        let signed = request.sign(&dummy_secret_key()).unwrap();

        assert_eq!(expected, *signed.transaction_hash());
    }

    #[test]
    fn test_eip155_signed_transaction_rlp() {
        let request = dummy_request();
        let signed = request.sign(&dummy_secret_key()).unwrap();

        let encoded = alloy_rlp::encode(&signed);
        let decoded = PreOrPostEip155::decode(&mut encoded.as_slice()).unwrap();
        let decoded = match decoded {
            PreOrPostEip155::Pre(_) => panic!("Expected post-EIP-155 transaction"),
            PreOrPostEip155::Post(post) => post,
        };

        assert_eq!(signed, decoded);
    }
}
