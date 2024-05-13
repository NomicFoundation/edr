// Part of this code was inspired by foundry. For the original context see:
// https://github.com/foundry-rs/foundry/blob/01b16238ff87dc7ca8ee3f5f13e389888c2a2ee4/anvil/core/src/eth/transaction/mod.rs
#![allow(missing_docs)]

//! transaction related data

mod fake_signature;
/// Types for transaction gossip (aka pooled transactions)
pub mod pooled;
mod request;
mod signed;
mod r#type;

pub use revm_primitives::alloy_primitives::TxKind;
use revm_primitives::B256;

pub use self::{r#type::TransactionType, request::*, signed::*};
use crate::{access_list::AccessListItem, Address, Bytes, U256};

pub trait Transaction {
    /// The effective gas price of the transaction, calculated using the
    /// provided block base fee.
    fn effective_gas_price(&self, block_base_fee: U256) -> U256;

    /// The maximum amount of gas the transaction can use.
    fn gas_limit(&self) -> u64;

    /// The gas price the sender is willing to pay.
    fn gas_price(&self) -> U256;

    /// The maximum fee per gas the sender is willing to pay. Only applicable
    /// for post-EIP-1559 transactions.
    fn max_fee_per_gas(&self) -> Option<U256>;

    /// The maximum fee per blob gas the sender is willing to pay. Only
    /// applicable for EIP-4844 transactions.
    fn max_fee_per_blob_gas(&self) -> Option<U256>;

    /// The maximum priority fee per gas the sender is willing to pay. Only
    /// applicable for post-EIP-1559 transactions.
    fn max_priority_fee_per_gas(&self) -> Option<U256>;

    /// The transaction's nonce.
    fn nonce(&self) -> u64;

    /// The address that receives the call, if any.
    fn to(&self) -> Option<Address>;

    /// The total amount of blob gas used by the transaction. Only applicable
    /// for EIP-4844 transactions.
    fn total_blob_gas(&self) -> Option<u64>;

    /// The hash of the transaction.
    fn transaction_hash(&self) -> &B256;

    /// The type of the transaction.
    fn transaction_type(&self) -> TransactionType;

    /// The value of the transaction.
    fn value(&self) -> U256;
}

pub fn max_cost(transaction: &impl Transaction) -> U256 {
    U256::from(transaction.gas_limit()).saturating_mul(transaction.gas_price())
}

pub fn upfront_cost(transaction: &impl Transaction) -> U256 {
    max_cost(transaction).saturating_add(transaction.value())
}

/// Represents _all_ transaction requests received from RPC
#[derive(Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct EthTransactionRequest {
    /// from address
    pub from: Address,
    /// to address
    #[cfg_attr(feature = "serde", serde(default))]
    pub to: Option<Address>,
    /// legacy, gas Price
    #[cfg_attr(feature = "serde", serde(default))]
    pub gas_price: Option<U256>,
    /// max base fee per gas sender is willing to pay
    #[cfg_attr(feature = "serde", serde(default))]
    pub max_fee_per_gas: Option<U256>,
    /// miner tip
    #[cfg_attr(feature = "serde", serde(default))]
    pub max_priority_fee_per_gas: Option<U256>,
    /// gas
    #[cfg_attr(feature = "serde", serde(default, with = "crate::serde::optional_u64"))]
    pub gas: Option<u64>,
    /// value of th tx in wei
    pub value: Option<U256>,
    /// Any additional data sent
    #[cfg_attr(feature = "serde", serde(alias = "input"))]
    pub data: Option<Bytes>,
    /// Transaction nonce
    #[cfg_attr(feature = "serde", serde(default, with = "crate::serde::optional_u64"))]
    pub nonce: Option<u64>,
    /// Chain ID
    #[cfg_attr(feature = "serde", serde(default, with = "crate::serde::optional_u64"))]
    pub chain_id: Option<u64>,
    /// warm storage access pre-payment
    #[cfg_attr(feature = "serde", serde(default))]
    pub access_list: Option<Vec<AccessListItem>>,
    /// EIP-2718 type
    #[cfg_attr(
        feature = "serde",
        serde(default, rename = "type", with = "crate::serde::optional_u64")
    )]
    pub transaction_type: Option<u64>,
    /// Blobs (EIP-4844)
    pub blobs: Option<Vec<Bytes>>,
    /// Blob versioned hashes (EIP-4844)
    pub blob_hashes: Option<Vec<B256>>,
}
