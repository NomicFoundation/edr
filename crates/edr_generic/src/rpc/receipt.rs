use edr_chain_l1::{receipt::L1BlockReceipt, rpc::receipt::L1RpcTransactionReceipt};
use edr_receipt::log::FilterLog;
use edr_rpc_spec::RpcTypeFrom;
use serde::{Deserialize, Serialize};

use crate::eip2718::TypedEnvelope;

#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    #[error("Legacy transaction is missing state root or status")]
    MissingStateRootOrStatus,
    #[error("Missing status")]
    MissingStatus,
}

use edr_receipt::{
    execution::{Eip658, Legacy},
    ExecutionReceipt, TransactionReceipt,
};
use edr_transaction::TransactionType;

// We need to introduce a newtype for BlockReceipt again due to the orphan rule,
// even though we use our own TypedEnvelope.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenericRpcTransactionReceipt(L1RpcTransactionReceipt);

impl TryFrom<GenericRpcTransactionReceipt>
    for L1BlockReceipt<TypedEnvelope<edr_receipt::Execution<FilterLog>>>
{
    type Error = ConversionError;

    fn try_from(value: GenericRpcTransactionReceipt) -> Result<Self, Self::Error> {
        let GenericRpcTransactionReceipt(value) = value;

        // We explicitly treat unknown transaction types as post-EIP 155 legacy
        // transactions
        let transaction_type = value.transaction_type.map_or(
            crate::transaction::Type::Legacy,
            crate::transaction::Type::from,
        );

        let execution = if transaction_type == crate::transaction::Type::Legacy {
            if let Some(status) = value.status {
                Eip658 {
                    status,
                    cumulative_gas_used: value.cumulative_gas_used,
                    logs_bloom: value.logs_bloom,
                    logs: value.logs,
                }
                .into()
            } else if let Some(state_root) = value.state_root {
                Legacy {
                    root: state_root,
                    cumulative_gas_used: value.cumulative_gas_used,
                    logs_bloom: value.logs_bloom,
                    logs: value.logs,
                }
                .into()
            } else {
                return Err(ConversionError::MissingStateRootOrStatus);
            }
        } else {
            Eip658 {
                status: value.status.ok_or(ConversionError::MissingStatus)?,
                cumulative_gas_used: value.cumulative_gas_used,
                logs_bloom: value.logs_bloom,
                logs: value.logs,
            }
            .into()
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
            },
        })
    }
}

impl RpcTypeFrom<L1BlockReceipt<TypedEnvelope<edr_receipt::Execution<FilterLog>>>>
    for GenericRpcTransactionReceipt
{
    type Hardfork = edr_chain_l1::Hardfork;

    fn rpc_type_from(
        value: &L1BlockReceipt<TypedEnvelope<edr_receipt::Execution<FilterLog>>>,
        hardfork: Self::Hardfork,
    ) -> Self {
        GenericRpcTransactionReceipt(L1RpcTransactionReceipt::rpc_type_from(value, hardfork))
    }
}
