use std::sync::OnceLock;

use alloy_rlp::{RlpDecodable, RlpEncodable};
use revm_primitives::{keccak256, AuthorizationList, TransactTo, TxEnv};

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
    pub access_list: AccessList,
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
