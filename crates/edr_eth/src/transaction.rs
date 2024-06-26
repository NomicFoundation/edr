// Part of this code was inspired by foundry. For the original context see:
// https://github.com/foundry-rs/foundry/blob/01b16238ff87dc7ca8ee3f5f13e389888c2a2ee4/anvil/core/src/eth/transaction/mod.rs
#![allow(missing_docs)]

//! transaction related data

mod fake_signature;
/// Types for transaction gossip (aka pooled transactions)
pub mod pooled;
/// Types for transaction requests.
pub mod request;
/// Types for signed transactions.
pub mod signed;
mod r#type;

use revm_primitives::B256;
pub use revm_primitives::{alloy_primitives::TxKind, Transaction, TransactionValidation};

pub use self::r#type::TransactionType;
use crate::{AccessListItem, Address, Bytes, U256};

pub const INVALID_TX_TYPE_ERROR_MESSAGE: &str = "invalid tx type";

/// Container type for various Ethereum transaction requests
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Request {
    /// A legacy transaction request
    Legacy(request::Legacy),
    /// An EIP-155 transaction request
    Eip155(request::Eip155),
    /// An EIP-2930 transaction request
    Eip2930(request::Eip2930),
    /// An EIP-1559 transaction request
    Eip1559(request::Eip1559),
    /// An EIP-4844 transaction request
    Eip4844(request::Eip4844),
}

/// Container type for various signed Ethereum transactions.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Signed {
    /// Legacy transaction
    PreEip155Legacy(signed::Legacy),
    /// EIP-155 transaction
    PostEip155Legacy(signed::Eip155),
    /// EIP-2930 transaction
    Eip2930(signed::Eip2930),
    /// EIP-1559 transaction
    Eip1559(signed::Eip1559),
    /// EIP-4844 transaction
    Eip4844(signed::Eip4844),
}

pub trait SignedTransaction: Transaction {
    /// The effective gas price of the transaction, calculated using the
    /// provided block base fee.
    fn effective_gas_price(&self, block_base_fee: U256) -> U256;

    /// The maximum fee per gas the sender is willing to pay. Only applicable
    /// for post-EIP-1559 transactions.
    fn max_fee_per_gas(&self) -> Option<U256>;

    /// The total amount of blob gas used by the transaction. Only applicable
    /// for EIP-4844 transactions.
    fn total_blob_gas(&self) -> Option<u64>;

    /// The hash of the transaction.
    fn transaction_hash(&self) -> &B256;

    /// The type of the transaction.
    fn transaction_type(&self) -> TransactionType;
}

pub trait TransactionMut {
    /// Sets the gas limit of the transaction.
    fn set_gas_limit(&mut self, gas_limit: u64);
}

pub fn max_cost(transaction: &impl SignedTransaction) -> U256 {
    U256::from(transaction.gas_limit()).saturating_mul(*transaction.gas_price())
}

pub fn upfront_cost(transaction: &impl SignedTransaction) -> U256 {
    max_cost(transaction).saturating_add(*transaction.value())
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
