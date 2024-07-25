use std::marker::PhantomData;

use edr_eth::{log::FilterLog, receipt::TransactionReceipt};

use super::BlockReceipt;
use crate::{eip2718::TypedEnvelope, receipt, transaction};

/// Error type for conversions from an RPC receipt to a typed receipt.
#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    /// Missing deposit nonce.
    ///
    /// Only occurs for deposit receipts.
    #[error("Missing deposit nonce")]
    MissingDepositNonce,
    /// Missing state root or status.
    ///
    /// Only occurs for legacy receipts.
    #[error("Legacy transaction is missing state root or status")]
    MissingStateRootOrStatus,
    /// Missing status.
    ///
    /// Only occurs for post-EIP-658 receipts.
    #[error("Missing status")]
    MissingStatus,
    /// Unknown transaction type.
    #[error("Unknown transaction type: {0}")]
    UnknownType(u8),
}

impl TryFrom<BlockReceipt> for receipt::BlockReceipt<TypedEnvelope<receipt::Execution<FilterLog>>> {
    type Error = ConversionError;

    fn try_from(value: BlockReceipt) -> Result<Self, Self::Error> {
        let transaction_type = value
            .transaction_type
            .map_or(Ok(transaction::Type::Legacy), transaction::Type::try_from)
            .map_err(ConversionError::UnknownType)?;

        let execution = match transaction_type {
            transaction::Type::Legacy => {
                if let Some(status) = value.status {
                    receipt::Execution::Eip658(receipt::execution::Eip658 {
                        status,
                        cumulative_gas_used: value.cumulative_gas_used,
                        logs_bloom: value.logs_bloom,
                        logs: value.logs,
                    })
                } else if let Some(state_root) = value.state_root {
                    receipt::Execution::Legacy(receipt::execution::Legacy {
                        root: state_root,
                        cumulative_gas_used: value.cumulative_gas_used,
                        logs_bloom: value.logs_bloom,
                        logs: value.logs,
                    })
                } else {
                    return Err(ConversionError::MissingStateRootOrStatus);
                }
            }
            transaction::Type::Deposit => {
                receipt::Execution::Deposit(receipt::execution::Deposit {
                    status: value.status.ok_or(ConversionError::MissingStatus)?,
                    cumulative_gas_used: value.cumulative_gas_used,
                    logs_bloom: value.logs_bloom,
                    logs: value.logs,
                    deposit_nonce: value
                        .deposit_nonce
                        .ok_or(ConversionError::MissingDepositNonce)?,
                    deposit_receipt_version: value.deposit_receipt_version,
                })
            }
            _ => receipt::Execution::Eip658(receipt::execution::Eip658 {
                status: value.status.ok_or(ConversionError::MissingStatus)?,
                cumulative_gas_used: value.cumulative_gas_used,
                logs_bloom: value.logs_bloom,
                logs: value.logs,
            }),
        };

        let enveloped = TypedEnvelope::new(execution, transaction_type);

        Ok(Self {
            block_hash: value.block_hash,
            block_number: value.block_number,
            inner: TransactionReceipt {
                inner: enveloped,
                transaction_hash: value.transaction_hash,
                transaction_index: value.transaction_index,
                from: value.from,
                to: value.to,
                contract_address: value.contract_address,
                gas_used: value.gas_used,
                effective_gas_price: value.effective_gas_price,
                phantom: PhantomData,
            },
        })
    }
}
