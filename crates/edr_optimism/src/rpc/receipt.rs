use std::marker::PhantomData;

use edr_eth::{
    log::FilterLog,
    receipt::{ExecutionReceipt as _, TransactionReceipt},
    transaction::TransactionType as _,
};
use edr_rpc_eth::RpcTypeFrom;
use revm_optimism::OptimismSpecId;

use super::BlockReceipt;
use crate::{eip2718::TypedEnvelope, receipt, transaction, OptimismChainSpec};

impl RpcTypeFrom<TransactionReceiptAndBlockForChainSpec<OptimismChainSpec>> for BlockReceipt {
    type Hardfork = OptimismSpecId;

    fn rpc_type_from(
        value: &BlockReceipt< TransactionReceiptAndBlockForChainSpec<OptimismChainSpec>,
        hardfork: Self::Hardfork,
    ) -> Self {
        let TransactionReceiptAndBlock { block, receipt } = value;

        let transaction_type = if hardfork >= OptimismSpecId::BERLIN {
            Some(u8::from(receipt.inner.transaction_type()))
        } else {
            None
        };

        Self {
            block_hash: receipt.block_hash,
            block_number: receipt.block_number,
            transaction_hash: receipt.inner.transaction_hash,
            transaction_index: receipt.inner.transaction_index,
            transaction_type,
            from: receipt.inner.from,
            to: receipt.inner.to,
            cumulative_gas_used: receipt.inner.cumulative_gas_used(),
            gas_used: receipt.inner.gas_used,
            contract_address: receipt.inner.contract_address,
            logs: receipt.inner.transaction_logs().to_vec(),
            logs_bloom: *receipt.inner.logs_bloom(),
            state_root: match receipt.inner.as_execution_receipt().data() {
                receipt::Execution::Legacy(receipt) => Some(receipt.root),
                receipt::Execution::Eip658(_) | receipt::Execution::Deposit(_) => None,
            },
            status: match receipt.inner.as_execution_receipt().data() {
                receipt::Execution::Legacy(_) => None,
                receipt::Execution::Eip658(receipt) => Some(receipt.status),
                receipt::Execution::Deposit(receipt) => Some(receipt.status),
            },
            effective_gas_price: receipt.inner.effective_gas_price,
            deposit_nonce: match receipt.inner.as_execution_receipt().data() {
                receipt::Execution::Legacy(_) | receipt::Execution::Eip658(_) => None,
                receipt::Execution::Deposit(receipt) => Some(receipt.deposit_nonce),
            },
            deposit_receipt_version: match receipt.inner.as_execution_receipt().data() {
                receipt::Execution::Legacy(_) | receipt::Execution::Eip658(_) => None,
                receipt::Execution::Deposit(receipt) => receipt.deposit_receipt_version,
            },
            l1_block_info: block.l1_block_info().clone(),
            authorization_list: None,
        }
    }
}

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

#[cfg(test)]
mod tests {
    use edr_eth::{log::ExecutionLog, Bloom, Bytes};
    use edr_rpc_eth::impl_execution_receipt_tests;

    use super::*;
    use crate::OptimismChainSpec;

    impl_execution_receipt_tests! {
        OptimismChainSpec => {
            legacy => TypedEnvelope::Legacy(receipt::Execution::Legacy(receipt::execution::Legacy {
                root: B256::random(),
                cumulative_gas_used: 0xffff,
                logs_bloom: Bloom::random(),
                logs: vec![
                    ExecutionLog::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                    ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::from_static(b"test"))
                ],
            })),
            eip658_eip2930 => TypedEnvelope::Eip2930(receipt::Execution::Eip658(receipt::execution::Eip658 {
                status: true,
                cumulative_gas_used: 0xffff,
                logs_bloom: Bloom::random(),
                logs: vec![
                    ExecutionLog::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                    ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::from_static(b"test"))
                ],
            })),
            eip658_eip1559 => TypedEnvelope::Eip2930(receipt::Execution::Eip658(receipt::execution::Eip658 {
                status: true,
                cumulative_gas_used: 0xffff,
                logs_bloom: Bloom::random(),
                logs: vec![
                    ExecutionLog::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                    ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::from_static(b"test"))
                ],
            })),
            eip658_eip4844 => TypedEnvelope::Eip4844(receipt::Execution::Eip658(receipt::execution::Eip658 {
                status: true,
                cumulative_gas_used: 0xffff,
                logs_bloom: Bloom::random(),
                logs: vec![
                    ExecutionLog::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                    ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::from_static(b"test"))
                ],
            })),
            deposit => TypedEnvelope::Deposit(receipt::Execution::Deposit(receipt::execution::Deposit {
                status: true,
                cumulative_gas_used: 0xffff,
                logs_bloom: Bloom::random(),
                logs: vec![
                    ExecutionLog::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                    ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::from_static(b"test"))
                ],
                deposit_nonce: 0x1234,
                deposit_receipt_version: Some(1u8),
            })),
        }
    }
}
