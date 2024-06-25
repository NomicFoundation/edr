use std::ops::Deref;

use edr_eth::{B256, U128};

/// Optimism RPC transaction.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    #[serde(flatten)]
    l1: edr_rpc_eth::Transaction,
    /// Hash that uniquely identifies the source of the deposit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_hash: Option<B256>,
    /// The ETH value to mint on L2
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mint: Option<U128>,
    /// Field indicating whether the transaction is a system transaction, and
    /// therefore exempt from the L2 gas limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_system_tx: Option<bool>,
}

impl Deref for Transaction {
    type Target = edr_rpc_eth::Transaction;

    fn deref(&self) -> &Self::Target {
        &self.l1
    }
}
