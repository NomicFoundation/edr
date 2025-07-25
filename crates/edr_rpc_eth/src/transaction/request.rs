use edr_eth::{
    eips::{eip2930, eip7702},
    Address, Blob, Bytes, B256, U256,
};

/// Represents _all_ transaction requests received from RPC
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionRequest {
    /// from address
    pub from: Address,
    /// to address
    #[serde(default)]
    pub to: Option<Address>,
    /// legacy, gas Price
    #[serde(default, with = "alloy_serde::quantity::opt")]
    pub gas_price: Option<u128>,
    /// max base fee per gas sender is willing to pay
    #[serde(default, with = "alloy_serde::quantity::opt")]
    pub max_fee_per_gas: Option<u128>,
    /// miner tip
    #[serde(default, with = "alloy_serde::quantity::opt")]
    pub max_priority_fee_per_gas: Option<u128>,
    /// gas
    #[serde(default, with = "alloy_serde::quantity::opt")]
    pub gas: Option<u64>,
    /// value of th tx in wei
    pub value: Option<U256>,
    /// Any additional data sent
    #[serde(alias = "input")]
    pub data: Option<Bytes>,
    /// Transaction nonce
    #[serde(default, with = "alloy_serde::quantity::opt")]
    pub nonce: Option<u64>,
    /// Chain ID
    #[serde(default, with = "alloy_serde::quantity::opt")]
    pub chain_id: Option<u64>,
    /// warm storage access pre-payment
    #[serde(default)]
    pub access_list: Option<Vec<eip2930::AccessListItem>>,
    /// EIP-2718 type
    #[serde(default, rename = "type", with = "alloy_serde::quantity::opt")]
    pub transaction_type: Option<u8>,
    /// Blobs (EIP-4844)
    pub blobs: Option<Vec<Blob>>,
    /// Blob versioned hashes (EIP-4844)
    pub blob_hashes: Option<Vec<B256>>,
    /// Authorizations are used to temporarily set the code of its signer to
    /// the code referenced by `address`. These also include a `chain_id` (which
    /// can be set to zero and not evaluated) as well as an optional `nonce`.
    #[serde(default)]
    pub authorization_list: Option<Vec<eip7702::SignedAuthorization>>,
}
