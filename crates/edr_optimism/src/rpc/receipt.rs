use std::marker::PhantomData;

use edr_eth::{
    log::FilterLog,
    receipt::{ExecutionReceipt as _, TransactionReceipt},
    transaction::TransactionType as _,
};
use edr_rpc_eth::RpcTypeFrom;
use op_alloy_rpc_types::receipt::L1BlockInfo;
use revm_optimism::OptimismSpecId;

use crate::{eip2718::TypedEnvelope, receipt, rpc, transaction};

impl RpcTypeFrom<receipt::Block> for rpc::BlockReceipt {
    type Hardfork = OptimismSpecId;

    fn rpc_type_from(value: &receipt::Block, hardfork: Self::Hardfork) -> Self {
        let transaction_type = if hardfork >= OptimismSpecId::BERLIN {
            Some(u8::from(value.eth.inner.transaction_type()))
        } else {
            None
        };

        Self {
            block_hash: value.eth.block_hash,
            block_number: value.eth.block_number,
            transaction_hash: value.eth.inner.transaction_hash,
            transaction_index: value.eth.inner.transaction_index,
            transaction_type,
            from: value.eth.inner.from,
            to: value.eth.inner.to,
            cumulative_gas_used: value.eth.inner.cumulative_gas_used(),
            gas_used: value.eth.inner.gas_used,
            contract_address: value.eth.inner.contract_address,
            logs: value.eth.inner.transaction_logs().to_vec(),
            logs_bloom: *value.eth.inner.logs_bloom(),
            state_root: match value.eth.inner.as_execution_receipt().data() {
                receipt::Execution::Legacy(receipt) => Some(receipt.root),
                receipt::Execution::Eip658(_) | receipt::Execution::Deposit(_) => None,
            },
            status: match value.eth.inner.as_execution_receipt().data() {
                receipt::Execution::Legacy(_) => None,
                receipt::Execution::Eip658(receipt) => Some(receipt.status),
                receipt::Execution::Deposit(receipt) => Some(receipt.status),
            },
            effective_gas_price: value.eth.inner.effective_gas_price,
            deposit_nonce: match value.eth.inner.as_execution_receipt().data() {
                receipt::Execution::Legacy(_) | receipt::Execution::Eip658(_) => None,
                receipt::Execution::Deposit(receipt) => Some(receipt.deposit_nonce),
            },
            deposit_receipt_version: match value.eth.inner.as_execution_receipt().data() {
                receipt::Execution::Legacy(_) | receipt::Execution::Eip658(_) => None,
                receipt::Execution::Deposit(receipt) => receipt.deposit_receipt_version,
            },
            l1_block_info: L1BlockInfo {
                l1_gas_price: value.l1_block_info.as_ref().map(|l1_block_info| {
                    l1_block_info
                        .l1_base_fee
                        .try_into()
                        .expect("L1 gas price cannot be larger than u128::max")
                }),
                // Not supported, as it was deprecated post-Fjord
                l1_gas_used: None,
                l1_fee: value.l1_block_info.as_ref().map(|l1_block_info| {
                    l1_block_info
                        .l1_base_fee
                        .try_into()
                        .expect("L1 fee cannot be larger than u128::max")
                }),
                // Not supported, as it was deprecated post-Ecotone
                l1_fee_scalar: None,
                l1_base_fee_scalar: value.l1_block_info.as_ref().map(|l1_block_info| {
                    l1_block_info
                        .l1_base_fee_scalar
                        .try_into()
                        .expect("L1 base fee scalar cannot be larger than u128::max")
                }),
                l1_blob_base_fee: value.l1_block_info.as_ref().and_then(|l1_block_info| {
                    l1_block_info.l1_blob_base_fee.map(|scalar| {
                        scalar
                            .try_into()
                            .expect("L1 blob base fee cannot be larger than u128::max")
                    })
                }),
                l1_blob_base_fee_scalar: value.l1_block_info.as_ref().and_then(|l1_block_info| {
                    l1_block_info.l1_blob_base_fee_scalar.map(|scalar| {
                        scalar
                            .try_into()
                            .expect("L1 blob base fee scalar cannot be larger than u128::max")
                    })
                }),
            },
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
    /// Missing L1 block info.
    ///
    /// Only occurs for non-deposit receipts.
    #[error("Missing L1 block info for a non-deposit receipt")]
    MissingL1BlockInfo,
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

impl TryFrom<rpc::BlockReceipt> for receipt::Block {
    type Error = ConversionError;

    fn try_from(value: rpc::BlockReceipt) -> Result<Self, Self::Error> {
        let transaction_type = value
            .transaction_type
            .map_or(Ok(transaction::Type::Legacy), transaction::Type::try_from)
            .map_err(ConversionError::UnknownType)?;

        let (execution, l1_block_info) = match transaction_type {
            transaction::Type::Legacy => {
                let execution = if let Some(status) = value.status {
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
                };

                (execution, None)
            }
            transaction::Type::Deposit => {
                let execution = receipt::Execution::Deposit(receipt::execution::Deposit {
                    status: value.status.ok_or(ConversionError::MissingStatus)?,
                    cumulative_gas_used: value.cumulative_gas_used,
                    logs_bloom: value.logs_bloom,
                    logs: value.logs,
                    deposit_nonce: value
                        .deposit_nonce
                        .ok_or(ConversionError::MissingDepositNonce)?,
                    deposit_receipt_version: value.deposit_receipt_version,
                });

                (execution, None)
            }
            _ => {
                let l1_block_info = crate::L1BlockInfo {
                    l1_base_fee: todo!(),
                    l1_fee_overhead: todo!(),
                    l1_base_fee_scalar: todo!(),
                    l1_blob_base_fee: todo!(),
                    l1_blob_base_fee_scalar: todo!(),
                };

                let execution = receipt::Execution::Eip658(receipt::execution::Eip658 {
                    status: value.status.ok_or(ConversionError::MissingStatus)?,
                    cumulative_gas_used: value.cumulative_gas_used,
                    logs_bloom: value.logs_bloom,
                    logs: value.logs,
                });

                (execution, Some(l1_block_info))
            }
        };

        let enveloped = TypedEnvelope::new(execution, transaction_type);

        let eth = edr_eth::receipt::BlockReceipt {
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
            },
        };

        Ok(Self { eth, l1_block_info })
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
