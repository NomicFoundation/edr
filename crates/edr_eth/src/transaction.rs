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

use std::str::FromStr;

pub use revm_primitives::{alloy_primitives::TxKind, Transaction, TransactionValidation};
use revm_primitives::{ruint, B256};

use crate::{AccessListItem, Address, Bytes, U256, U8};

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

/// The type of transaction.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Type {
    /// Legacy transaction
    Legacy = signed::Legacy::TYPE,
    /// EIP-2930 transaction
    Eip2930 = signed::Eip2930::TYPE,
    /// EIP-1559 transaction
    Eip1559 = signed::Eip1559::TYPE,
    /// EIP-4844 transaction
    Eip4844 = signed::Eip4844::TYPE,
}

impl From<Type> for u8 {
    fn from(t: Type) -> u8 {
        t as u8
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("{0}")]
    BaseConvertError(ruint::BaseConvertError),
    #[error("Invalid digit: {0}")]
    InvalidDigit(char),
    #[error("Invalid radix. Only hexadecimal is supported.")]
    InvalidRadix,
    #[error("Unknown transaction type: {0}")]
    UnknownType(u8),
}

impl From<ruint::ParseError> for ParseError {
    fn from(error: ruint::ParseError) -> Self {
        match error {
            ruint::ParseError::InvalidDigit(c) => ParseError::InvalidDigit(c),
            ruint::ParseError::InvalidRadix(_) => ParseError::InvalidRadix,
            ruint::ParseError::BaseConvertError(error) => ParseError::BaseConvertError(error),
        }
    }
}

impl FromStr for Type {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_char_boundary(2) {
            let (prefix, rest) = s.split_at(2);
            if prefix == "0x" {
                let value = U8::from_str_radix(rest, 16)?;

                Type::try_from(value.to::<u8>()).map_err(ParseError::UnknownType)
            } else {
                Err(ParseError::InvalidRadix)
            }
        } else {
            Err(ParseError::InvalidRadix)
        }
    }
}

impl TryFrom<u8> for Type {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            signed::Legacy::TYPE => Ok(Self::Legacy),
            signed::Eip2930::TYPE => Ok(Self::Eip2930),
            signed::Eip1559::TYPE => Ok(Self::Eip1559),
            signed::Eip4844::TYPE => Ok(Self::Eip4844),
            value => Err(value),
        }
    }
}

#[cfg(feature = "serde")]
impl<'deserializer> serde::Deserialize<'deserializer> for Type {
    fn deserialize<D>(deserializer: D) -> Result<Type, D::Error>
    where
        D: serde::Deserializer<'deserializer>,
    {
        let value = U8::deserialize(deserializer)?;
        Type::try_from(value.to::<u8>()).map_err(serde::de::Error::custom)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Type {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        U8::serialize(&U8::from(u8::from(*self)), serializer)
    }
}

pub trait SignedTransaction: Transaction + TransactionType {
    /// The effective gas price of the transaction, calculated using the
    /// provided block base fee. Only applicable for post-EIP-1559 transactions.
    fn effective_gas_price(&self, block_base_fee: U256) -> Option<U256>;

    /// The maximum fee per gas the sender is willing to pay. Only applicable
    /// for post-EIP-1559 transactions.
    fn max_fee_per_gas(&self) -> Option<U256>;

    /// The enveloped (EIP-2718) RLP-encoding of the transaction.
    fn rlp_encoding(&self) -> &Bytes;

    /// The total amount of blob gas used by the transaction. Only applicable
    /// for EIP-4844 transactions.
    fn total_blob_gas(&self) -> Option<u64>;

    /// The hash of the transaction.
    fn transaction_hash(&self) -> &B256;
}

pub trait TransactionMut {
    /// Sets the gas limit of the transaction.
    fn set_gas_limit(&mut self, gas_limit: u64);
}

pub trait TransactionType {
    /// Type of the transaction.
    type Type;

    /// Returns the type of the transaction.
    fn transaction_type(&self) -> Self::Type;
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
        serde(default, rename = "type", with = "crate::serde::optional_u8")
    )]
    pub transaction_type: Option<u8>,
    /// Blobs (EIP-4844)
    pub blobs: Option<Vec<Bytes>>,
    /// Blob versioned hashes (EIP-4844)
    pub blob_hashes: Option<Vec<B256>>,
}
