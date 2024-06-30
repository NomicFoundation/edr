use std::sync::OnceLock;

use alloy_rlp::{RlpDecodable, RlpEncodable};
use revm_primitives::keccak256;

use crate::{
    signature::{self, Fakeable},
    transaction::{self, TxKind},
    Address, Bytes, B256, U256,
};

#[derive(Clone, Debug, Eq, RlpEncodable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Legacy {
    // The order of these fields determines encoding order.
    #[cfg_attr(feature = "serde", serde(with = "crate::serde::u64"))]
    pub nonce: u64,
    pub gas_price: U256,
    #[cfg_attr(feature = "serde", serde(with = "crate::serde::u64"))]
    pub gas_limit: u64,
    pub kind: TxKind,
    pub value: U256,
    pub input: Bytes,
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub signature: signature::Fakeable<signature::SignatureWithRecoveryId>,
    /// Cached transaction hash
    #[rlp(skip)]
    #[cfg_attr(feature = "serde", serde(skip))]
    pub hash: OnceLock<B256>,
}

impl Legacy {
    /// The type identifier for a pre-EIP-155 legacy transaction.
    pub const TYPE: u8 = transaction::request::Legacy::TYPE;

    /// Returns the caller/signer of the transaction.
    pub fn caller(&self) -> &Address {
        self.signature.caller()
    }

    pub fn transaction_hash(&self) -> &B256 {
        self.hash.get_or_init(|| keccak256(alloy_rlp::encode(self)))
    }
}

impl PartialEq for Legacy {
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

/// A transaction that is either a legacy transaction or an EIP-155
/// transaction. This is used to decode `super::Signed`, as
/// their decoding format is the same.
pub enum PreOrPostEip155 {
    Pre(Legacy),
    Post(transaction::signed::Eip155),
}

impl alloy_rlp::Decodable for PreOrPostEip155 {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        #[derive(RlpDecodable)]
        struct Decodable {
            // The order of these fields determines decoding order.
            pub nonce: u64,
            pub gas_price: U256,
            pub gas_limit: u64,
            pub kind: TxKind,
            pub value: U256,
            pub input: Bytes,
            pub signature: signature::SignatureWithRecoveryId,
        }

        impl From<&Decodable> for transaction::request::Eip155 {
            fn from(value: &Decodable) -> Self {
                let chain_id = transaction::signed::eip155::v_to_chain_id(value.signature.v);
                Self {
                    nonce: value.nonce,
                    gas_price: value.gas_price,
                    gas_limit: value.gas_limit,
                    kind: value.kind,
                    value: value.value,
                    input: value.input.clone(),
                    chain_id,
                }
            }
        }

        impl From<&Decodable> for transaction::request::Legacy {
            fn from(value: &Decodable) -> Self {
                Self {
                    nonce: value.nonce,
                    gas_price: value.gas_price,
                    gas_limit: value.gas_limit,
                    kind: value.kind,
                    value: value.value,
                    input: value.input.clone(),
                }
            }
        }

        let transaction = Decodable::decode(buf)?;

        let transaction = if transaction.signature.v >= 35 {
            let request = transaction::request::Eip155::from(&transaction);

            let signature = Fakeable::recover(transaction.signature, request.hash().into())
                .map_err(|_error| alloy_rlp::Error::Custom("Invalid Signature"))?;

            Self::Post(transaction::signed::Eip155 {
                nonce: transaction.nonce,
                gas_price: transaction.gas_price,
                gas_limit: transaction.gas_limit,
                kind: transaction.kind,
                value: transaction.value,
                input: transaction.input,
                signature,
                hash: OnceLock::new(),
            })
        } else {
            let request = transaction::request::Legacy::from(&transaction);

            let signature = Fakeable::recover(transaction.signature, request.hash().into())
                .map_err(|_error| alloy_rlp::Error::Custom("Invalid Signature"))?;

            Self::Pre(Legacy {
                nonce: request.nonce,
                gas_price: request.gas_price,
                gas_limit: request.gas_limit,
                kind: request.kind,
                value: request.value,
                input: request.input,
                signature,
                hash: OnceLock::new(),
            })
        };

        Ok(transaction)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_rlp::Decodable as _;
    use k256::SecretKey;

    use super::*;
    use crate::signature::secret_key_from_str;

    fn dummy_request() -> transaction::request::Legacy {
        let to = Address::from_str("0xc014ba5ec014ba5ec014ba5ec014ba5ec014ba5e").unwrap();
        let input = hex::decode("1234").unwrap();
        transaction::request::Legacy {
            nonce: 1,
            gas_price: U256::from(2),
            gas_limit: 3,
            kind: TxKind::Call(to),
            value: U256::from(4),
            input: Bytes::from(input),
        }
    }

    fn dummy_secret_key() -> SecretKey {
        secret_key_from_str("e331b6d69882b4cb4ea581d88e0b604039a3de5967688d3dcffdd2270c0fd109")
            .unwrap()
    }

    #[test]
    fn test_legacy_signed_transaction_encoding() {
        // Generated by Hardhat
        let expected =
            hex::decode("f85f01020394c014ba5ec014ba5ec014ba5ec014ba5ec014ba5e048212341ca0c62d73a484ff7c53a0cfdf8eaa5e5896491b70971e9ce4a3e8750772b7c0203fa00562866909572aee9ab72df7470c1dd7aa29b056597be57c17e06f1ee303e7eb").unwrap();

        let request = dummy_request();
        let signed = request.sign(&dummy_secret_key()).unwrap();

        let encoded = alloy_rlp::encode(&signed);
        assert_eq!(expected, encoded);
    }

    #[test]
    fn test_legacy_signed_transaction_hash() {
        // Generated by hardhat
        let expected = B256::from_slice(
            &hex::decode("854a9427d54aaca361e7c592b4c3dc7da279c52a00cad157dab0365dcc27578d")
                .unwrap(),
        );

        let request = dummy_request();
        let signed = request.sign(&dummy_secret_key()).unwrap();

        assert_eq!(expected, *signed.transaction_hash());
    }

    #[test]
    fn test_legacy_signed_transaction_rlp() {
        let request = dummy_request();
        let signed = request.sign(&dummy_secret_key()).unwrap();

        let encoded = alloy_rlp::encode(&signed);
        let decoded = PreOrPostEip155::decode(&mut encoded.as_slice()).unwrap();
        let decoded = match decoded {
            PreOrPostEip155::Pre(pre) => pre,
            PreOrPostEip155::Post(_) => panic!("Expected pre-EIP-155 transaction"),
        };

        assert_eq!(signed, decoded);
    }
}
