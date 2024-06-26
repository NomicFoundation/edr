use std::sync::OnceLock;

use edr_eth::{B256, U128};
use edr_evm::transaction::{remote::EthRpcTransaction, TxKind};
use edr_rpc_eth::TransactionConversionError as L1ConversionError;

use crate::transaction;

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

impl Transaction {
    /// Returns whether the transaction is a legacy transaction.
    pub fn is_legacy(&self) -> bool {
        matches!(self.l1.transaction_type, None | Some(0)) && matches!(self.l1.v, 27 | 28)
    }
}

impl EthRpcTransaction for Transaction {
    fn block_hash(&self) -> Option<&B256> {
        self.l1.block_hash()
    }
}

impl TryFrom<Transaction> for transaction::signed::Deposited {
    type Error = ConversionError;

    fn try_from(value: Transaction) -> Result<Self, Self::Error> {
        let transaction = Self {
            source_hash: value.source_hash.ok_or(ConversionError::SourceHash)?,
            from: value.l1.from,
            to: if let Some(to) = value.l1.to {
                TxKind::Call(to)
            } else {
                TxKind::Create
            },
            mint: value.mint.map_or(0, |mint| mint.to()),
            value: value.l1.value,
            gas_limit: value.l1.gas.to(),
            is_system_tx: value.is_system_tx.unwrap_or(false),
            data: value.l1.input,
            hash: OnceLock::new(),
        };

        Ok(transaction)
    }
}

impl TryFrom<Transaction> for transaction::Signed {
    type Error = ConversionError;

    fn try_from(value: Transaction) -> Result<Self, Self::Error> {
        let transaction_type = match value
            .l1
            .transaction_type
            .map_or(Ok(transaction::Type::Legacy), transaction::Type::try_from)
        {
            Ok(r#type) => r#type,
            Err(r#type) => {
                log::warn!("Unsupported transaction type: {type}. Reverting to post-EIP 155 legacy transaction");

                // As the transaction type is not 0 or `None`, this will always result in a
                // post-EIP 155 legacy transaction.
                transaction::Type::Legacy
            }
        };

        let transaction = match transaction_type {
            transaction::Type::Legacy => {
                if value.is_legacy() {
                    Self::PreEip155Legacy(value.l1.into())
                } else {
                    Self::PostEip155Legacy(value.l1.into())
                }
            }
            transaction::Type::Eip2930 => Self::Eip2930(value.l1.try_into()?),
            transaction::Type::Eip1559 => Self::Eip1559(value.l1.try_into()?),
            transaction::Type::Eip4844 => Self::Eip4844(value.l1.try_into()?),
            transaction::Type::Deposited => Self::Deposited(value.try_into()?),
        };

        Ok(transaction)
    }
}

/// Error that occurs when trying to convert the JSON-RPC `Transaction` type.
#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    /// L1 conversion error
    #[error(transparent)]
    L1(#[from] L1ConversionError),
    /// Missing mint
    #[error("Missing mint")]
    Mint,
    /// Missing source hash
    #[error("Missing source hash")]
    SourceHash,
}
