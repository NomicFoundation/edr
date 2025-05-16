//! Ethereum withdrawal type

use alloy_rlp::{RlpDecodable, RlpEncodable};

use crate::{Address, U256};

/// Ethereum withdrawal
#[derive(Clone, Debug, PartialEq, Eq, RlpDecodable, RlpEncodable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Withdrawal {
    /// The index of withdrawal
    #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
    pub index: u64,
    /// The index of the validator that generated the withdrawal
    #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
    pub validator_index: u64,
    /// The recipient address for withdrawal value
    pub address: Address,
    /// The value contained in withdrawal
    pub amount: U256,
}
