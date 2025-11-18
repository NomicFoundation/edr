//! Types for OP execution receipts.

mod deposit;

use edr_block_header::PartialHeader;
use edr_chain_spec_evm::result::ExecutionResult;
use edr_primitives::Bloom;
pub use edr_receipt::execution::{Eip658, Legacy};
use edr_receipt::{
    log::{logs_to_bloom, ExecutionLog},
    ExecutionReceipt, MapReceiptLogs, RootOrStatus,
};
use edr_receipt_builder_api::ExecutionReceiptBuilder;
use edr_state_api::State;
use edr_transaction::{Transaction as _, TransactionType as _};

pub use self::deposit::Deposit;
use self::deposit::Eip658OrDeposit;
use crate::{
    eip2718::TypedEnvelope,
    transaction::{signed::OpSignedTransaction, OpTransactionType},
    HaltReason, Hardfork,
};

/// OP execution receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpExecutionReceipt<LogT> {
    /// EIP-658 receipt.
    Eip658(edr_receipt::execution::Eip658<LogT>),
    /// OP deposit receipt (post-Regolith).
    Deposit(Deposit<LogT>),
}

impl<LogT> From<Legacy<LogT>> for OpExecutionReceipt<LogT> {
    fn from(value: Legacy<LogT>) -> Self {
        OpExecutionReceipt::Eip658(value.into())
    }
}

impl<LogT> From<Eip658<LogT>> for OpExecutionReceipt<LogT> {
    fn from(value: Eip658<LogT>) -> Self {
        OpExecutionReceipt::Eip658(value)
    }
}

impl<LogT> From<Deposit<LogT>> for OpExecutionReceipt<LogT> {
    fn from(value: Deposit<LogT>) -> Self {
        OpExecutionReceipt::Deposit(value)
    }
}

impl<LogT> alloy_rlp::Decodable for OpExecutionReceipt<LogT>
where
    LogT: alloy_rlp::Decodable,
{
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        // Use a temporary buffer to decode the header, avoiding the original buffer
        // from being advanced
        let first_value_header = {
            let mut temp_buf = *buf;

            let _receipt_header = alloy_rlp::Header::decode(&mut temp_buf)?;
            alloy_rlp::Header::decode(&mut temp_buf)?
        };

        // The first value of the receipt is 1 byte long, which means it's the status
        // code of an EIP-658 receipt.
        if first_value_header.payload_length == 1 {
            let receipt = Eip658OrDeposit::decode(buf)?;
            Ok(receipt.into())
        } else {
            let receipt = Legacy::<LogT>::decode(buf)?;
            Ok(receipt.into())
        }
    }
}

impl<LogT> alloy_rlp::Encodable for OpExecutionReceipt<LogT>
where
    LogT: alloy_rlp::Encodable,
{
    fn encode(&self, out: &mut dyn alloy_rlp::BufMut) {
        match self {
            OpExecutionReceipt::Eip658(receipt) => receipt.encode(out),
            OpExecutionReceipt::Deposit(receipt) => receipt.encode(out),
        }
    }

    fn length(&self) -> usize {
        match self {
            OpExecutionReceipt::Eip658(receipt) => receipt.length(),
            OpExecutionReceipt::Deposit(receipt) => receipt.length(),
        }
    }
}

/// OP execution receipt builder.
pub struct OpExecutionReceiptBuilder {
    deposit_nonce: u64,
}

impl ExecutionReceiptBuilder<HaltReason, Hardfork, OpSignedTransaction>
    for OpExecutionReceiptBuilder
{
    type Receipt = TypedEnvelope<OpExecutionReceipt<ExecutionLog>>;

    fn new_receipt_builder<StateT: State>(
        pre_execution_state: StateT,
        transaction: &OpSignedTransaction,
    ) -> Result<Self, StateT::Error> {
        let deposit_nonce = pre_execution_state
            .basic(transaction.caller())?
            .map_or(0, |account| account.nonce);

        Ok(Self { deposit_nonce })
    }

    fn build_receipt(
        self,
        header: &PartialHeader,
        transaction: &OpSignedTransaction,
        result: &ExecutionResult<HaltReason>,
        hardfork: Hardfork,
    ) -> Self::Receipt {
        let logs = result.logs().to_vec();
        let logs_bloom = logs_to_bloom(&logs);

        let receipt = if transaction.transaction_type() == OpTransactionType::Deposit {
            OpExecutionReceipt::Deposit(Deposit {
                status: result.is_success(),
                cumulative_gas_used: header.gas_used,
                logs_bloom,
                logs,
                deposit_nonce: self.deposit_nonce,
                deposit_receipt_version: if hardfork >= Hardfork::CANYON {
                    Some(1)
                } else {
                    None
                },
            })
        } else {
            OpExecutionReceipt::Eip658(Eip658 {
                status: result.is_success(),
                cumulative_gas_used: header.gas_used,
                logs_bloom,
                logs,
            })
        };

        TypedEnvelope::new(receipt, transaction.transaction_type())
    }
}

impl<LogT, NewLogT> MapReceiptLogs<LogT, NewLogT, OpExecutionReceipt<NewLogT>>
    for OpExecutionReceipt<LogT>
{
    fn map_logs(self, map_fn: impl FnMut(LogT) -> NewLogT) -> OpExecutionReceipt<NewLogT> {
        match self {
            OpExecutionReceipt::Eip658(receipt) => {
                OpExecutionReceipt::Eip658(receipt.map_logs(map_fn))
            }
            OpExecutionReceipt::Deposit(receipt) => {
                OpExecutionReceipt::Deposit(receipt.map_logs(map_fn))
            }
        }
    }
}

impl<LogT> ExecutionReceipt for OpExecutionReceipt<LogT> {
    type Log = LogT;

    fn cumulative_gas_used(&self) -> u64 {
        match self {
            OpExecutionReceipt::Eip658(receipt) => receipt.cumulative_gas_used,
            OpExecutionReceipt::Deposit(receipt) => receipt.cumulative_gas_used,
        }
    }

    fn logs_bloom(&self) -> &Bloom {
        match self {
            OpExecutionReceipt::Eip658(receipt) => &receipt.logs_bloom,
            OpExecutionReceipt::Deposit(receipt) => &receipt.logs_bloom,
        }
    }

    fn transaction_logs(&self) -> &[LogT] {
        match self {
            OpExecutionReceipt::Eip658(receipt) => &receipt.logs,
            OpExecutionReceipt::Deposit(receipt) => &receipt.logs,
        }
    }

    fn root_or_status(&self) -> RootOrStatus<'_> {
        match self {
            OpExecutionReceipt::Eip658(receipt) => RootOrStatus::Status(receipt.status),
            OpExecutionReceipt::Deposit(receipt) => RootOrStatus::Status(receipt.status),
        }
    }
}
#[cfg(test)]
mod tests {
    use alloy_rlp::Decodable as _;
    use edr_primitives::{Address, Bytes, B256};
    use edr_receipt::log::ExecutionLog;

    use super::*;
    use crate::eip2718::TypedEnvelope;

    macro_rules! impl_execution_receipt_tests {
        ($(
            $name:ident => $receipt:expr,
        )+) => {
            $(
                paste::item! {
                    #[test]
                    fn [<typed_receipt_rlp_encoding_ $name>]() -> anyhow::Result<()> {
                        let receipt = $receipt;

                        let encoded = alloy_rlp::encode(&receipt);
                        let decoded = TypedEnvelope::<OpExecutionReceipt::<ExecutionLog>>::decode(&mut encoded.as_slice())?;
                        assert_eq!(decoded, receipt);

                        Ok(())
                    }
                }
            )+
        };
    }

    impl_execution_receipt_tests! {
        eip658_legacy => TypedEnvelope::Legacy(OpExecutionReceipt::Eip658(Eip658 {
            status: true,
            cumulative_gas_used: 0xffff,
            logs_bloom: Bloom::random(),
            logs: vec![
                ExecutionLog::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::from_static(b"test"))
            ],
        })),
        eip658_eip2930 => TypedEnvelope::Eip2930(OpExecutionReceipt::Eip658(Eip658 {
            status: true,
            cumulative_gas_used: 0xffff,
            logs_bloom: Bloom::random(),
            logs: vec![
                ExecutionLog::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::from_static(b"test"))
            ],
        })),
        eip658_eip1559 => TypedEnvelope::Eip2930(OpExecutionReceipt::Eip658(Eip658 {
            status: true,
            cumulative_gas_used: 0xffff,
            logs_bloom: Bloom::random(),
            logs: vec![
                ExecutionLog::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::from_static(b"test"))
            ],
        })),
        eip658_eip4844 => TypedEnvelope::Eip4844(OpExecutionReceipt::Eip658(Eip658 {
            status: true,
            cumulative_gas_used: 0xffff,
            logs_bloom: Bloom::random(),
            logs: vec![
                ExecutionLog::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::from_static(b"test"))
            ],
        })),
        deposit => TypedEnvelope::Deposit(OpExecutionReceipt::Deposit(Deposit {
            status: true,
            cumulative_gas_used: 0xffff,
            logs_bloom: Bloom::random(),
            logs: vec![
                ExecutionLog::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                ExecutionLog::new_unchecked(Address::random(), Vec::new(),Bytes::from_static(b"test")),
            ],
            deposit_nonce: 0x1234,
            deposit_receipt_version: Some(0x01),
        })),
    }
}
