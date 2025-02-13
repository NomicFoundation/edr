use std::sync::OnceLock;

use alloy_consensus::{Signed, TxEip7702};
use alloy_rlp::{RlpDecodable, RlpEncodable};
use revm_primitives::{keccak256, AccessListItem, AuthorizationList, TransactTo, TxEnv};

use crate::{
    eips::eip7702,
    signature,
    transaction::{self, request, ComputeTransactionHash as _},
    utils::envelop_bytes,
    AccessList, Address, Bytes, B256, U256,
};

#[derive(Clone, Debug, Eq, RlpEncodable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Eip7702 {
    // The order of these fields determines encoding order.
    #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
    pub chain_id: u64,
    #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
    pub nonce: u64,
    pub max_priority_fee_per_gas: U256,
    pub max_fee_per_gas: U256,
    #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
    pub gas_limit: u64,
    pub to: Address,
    pub value: U256,
    pub input: Bytes,
    pub access_list: AccessList,
    pub authorization_list: Vec<eip7702::SignedAuthorization>,
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub signature: signature::Fakeable<signature::SignatureWithYParity>,
    /// Cached transaction hash
    #[rlp(default)]
    #[rlp(skip)]
    #[cfg_attr(feature = "serde", serde(skip))]
    pub hash: OnceLock<B256>,
}

impl Eip7702 {
    pub const TYPE: u8 = request::Eip7702::TYPE;

    /// Retrieves the caller/signer of the transaction.
    pub fn caller(&self) -> &Address {
        self.signature.caller()
    }

    /// Retrieves the cached transaction hash, if available. Otherwise, computes
    /// the hash and caches it.
    pub fn transaction_hash(&self) -> &B256 {
        self.hash.get_or_init(|| {
            let encoded = alloy_rlp::encode(self);
            let enveloped = envelop_bytes(Self::TYPE, &encoded);

            keccak256(enveloped)
        })
    }
}

impl From<Eip7702> for TxEnv {
    fn from(value: Eip7702) -> Self {
        TxEnv {
            caller: *value.caller(),
            gas_limit: value.gas_limit,
            gas_price: value.max_fee_per_gas,
            transact_to: TransactTo::Call(value.to),
            value: value.value,
            data: value.input,
            nonce: Some(value.nonce),
            chain_id: Some(value.chain_id),
            access_list: value.access_list.into(),
            gas_priority_fee: Some(value.max_priority_fee_per_gas),
            blob_hashes: Vec::new(),
            max_fee_per_blob_gas: None,
            authorization_list: Some(AuthorizationList::Signed(value.authorization_list)),
        }
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
    pub max_priority_fee_per_gas: U256,
    pub max_fee_per_gas: U256,
    pub gas_limit: u64,
    pub to: Address,
    pub value: U256,
    pub input: Bytes,
    pub access_list: Vec<AccessListItem>,
    pub authorization_list: Vec<eip7702::SignedAuthorization>,
    pub signature: signature::SignatureWithYParity,
}

impl alloy_rlp::Decodable for Eip7702 {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        // let buf2 = &mut *buf;
        // let tx = Signed::<TxEip7702>::rlp_decode(buf2)?;
        // println!("tx: {tx:?}");

        let transaction = Decodable::decode(buf)?;
        let request = transaction::request::Eip7702::from(&transaction);

        println!("signature: {:?}", transaction.signature);

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
            access_list: transaction.access_list.into(),
            authorization_list: transaction.authorization_list,
            signature,
            hash: OnceLock::new(),
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
            access_list: value.access_list.clone(),
            authorization_list: value.authorization_list.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    mod expectation {
        use core::str::FromStr as _;

        use edr_defaults::SECRET_KEYS;
        use hex::FromHexError;

        use super::*;
        use crate::signature::{secret_key_from_str, SecretKey, SignatureError};

        pub const TRANSACTION_HASH: B256 =
            b256!("b484d448147b9a6cafc732e01b89ee4e7d8bb783a03f5cbdd967d7bdaa945a99");
        // 0x063342b95531860d7257a1fb09a090d4858f1ab61843978fc29fb4154db9f392

        pub fn raw() -> Result<Vec<u8>, FromHexError> {
            hex::decode("04f8cc827a6980843b9aca00848321560082f61894f39fd6e51aad88f6f4ce6ab8827279cfffb922668080c0f85ef85c827a699412345678901234567890123456789012345678908080a0b776080626e62615e2a51a6bde9b4b4612af2627e386734f9af466ecfce19b8da00d5c886f5874383826ac237ea99bfbbf601fad0fd344458296677930d51ff44480a0a5f83207382081e8de07113af9ba61e4b41c9ae306edc55a2787996611d1ade9a0082f979b985ea64b4755344b57bcd66ade2b840e8be2036101d9cf23a8548412")
        }

        // Test vector generated using secret key in `dummy_secret_key`.
        pub fn request() -> anyhow::Result<transaction::request::Eip7702> {
            const CHAIN_ID: u64 = 0x7a69;

            let request = transaction::request::Eip7702 {
                chain_id: CHAIN_ID,
                nonce: 0,
                max_priority_fee_per_gas: U256::from(1_000_000_000u64),
                max_fee_per_gas: U256::from(2_200_000_000u64),
                gas_limit: 63_000,
                to: address!("0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"),
                value: U256::ZERO,
                input: Bytes::new(),
                access_list: Vec::new(),
                authorization_list: vec![eip7702::SignedAuthorization::new_unchecked(
                    eip7702::Authorization {
                        chain_id: U256::from(CHAIN_ID),
                        address: address!("0x1234567890123456789012345678901234567890"),
                        nonce: 0,
                    },
                    0,
                    U256::from_str(
                        "0xb776080626e62615e2a51a6bde9b4b4612af2627e386734f9af466ecfce19b8d",
                    )
                    .expect("R value is valid"),
                    U256::from_str(
                        "0x0d5c886f5874383826ac237ea99bfbbf601fad0fd344458296677930d51ff444",
                    )
                    .expect("S value is valid"),
                )],
            };
            Ok(request)
        }

        pub fn signed() -> anyhow::Result<transaction::Signed> {
            let request = expectation::request()?;

            let secret_key = expectation::secret_key()?;
            let signed = request.sign(&secret_key)?;
            println!("signed: {signed:?}");

            Ok(signed.into())
        }

        pub fn secret_key() -> Result<SecretKey, SignatureError> {
            secret_key_from_str(SECRET_KEYS[0])
        }
    }

    use alloy_rlp::Decodable as _;
    use revm_primitives::{address, b256};

    use super::*;
    use crate::transaction::Transaction as _;

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
}
