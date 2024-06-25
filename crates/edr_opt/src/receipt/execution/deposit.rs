use alloy_rlp::{RlpDecodable, RlpEncodable};
use edr_eth::{receipt::MapReceiptLogs, Bloom};

use super::{Deposit, Execution};
use crate::transaction;

#[derive(RlpDecodable)]
#[rlp(trailing)]
pub(super) struct Eip2718OrDeposit<LogT> {
    status: bool,
    cumulative_gas_used: u64,
    logs_bloom: Bloom,
    logs: Vec<LogT>,
    deposit_nonce: Option<u64>,
    deposit_receipt_version: Option<u8>,
}

impl<LogT> Eip2718OrDeposit<LogT> {
    /// Converts the instance into an execution receipt.
    pub fn into_execution_receipt(self, transaction_type: transaction::Type) -> Execution<LogT> {
        if let Some(deposit_none) = self.deposit_nonce {
            Execution::Deposit(Deposit {
                status: self.status,
                cumulative_gas_used: self.cumulative_gas_used,
                logs_bloom: self.logs_bloom,
                logs: self.logs,
                transaction_type,
                deposit_nonce: deposit_none,
                deposit_receipt_version: self.deposit_receipt_version,
            })
        } else {
            Execution::Eip2718(super::Eip2718 {
                status: self.status,
                cumulative_gas_used: self.cumulative_gas_used,
                logs_bloom: self.logs_bloom,
                logs: self.logs,
                transaction_type,
            })
        }
    }
}

#[derive(RlpEncodable)]
#[rlp(trailing)]
struct Encodable<'receipt, LogT> {
    status: bool,
    cumulative_gas_used: u64,
    logs_bloom: &'receipt Bloom,
    logs: &'receipt Vec<LogT>,
    deposit_nonce: Option<u64>,
    deposit_receipt_version: Option<u8>,
}

impl<'receipt, LogT> From<&'receipt Deposit<LogT>> for Encodable<'receipt, LogT> {
    fn from(receipt: &'receipt Deposit<LogT>) -> Self {
        Self {
            status: receipt.status,
            cumulative_gas_used: receipt.cumulative_gas_used,
            logs_bloom: &receipt.logs_bloom,
            logs: &receipt.logs,
            // Before Canyon, `deposit_nonce` is not present in the RLP-encoding of a receipt.
            // `deposit_receipt_version` is only present post-Canyon, so we can use it to determine
            // whether we're pre-Canyon.
            // Source: <https://specs.optimism.io/protocol/deposits.html#deposit-receipt>
            deposit_nonce: if receipt.deposit_receipt_version.is_some() {
                Some(receipt.deposit_nonce)
            } else {
                None
            },
            deposit_receipt_version: receipt.deposit_receipt_version,
        }
    }
}

impl<LogT> alloy_rlp::Encodable for Deposit<LogT>
where
    LogT: alloy_rlp::Encodable,
{
    fn encode(&self, out: &mut dyn alloy_rlp::BufMut) {
        out.put_u8(self.transaction_type.into());

        Encodable::from(self).encode(out);
    }

    fn length(&self) -> usize {
        1 + Encodable::from(self).length()
    }
}

// implement MapReceiptLogs for Deposit
impl<LogT, NewLogT> MapReceiptLogs<LogT, NewLogT, Deposit<NewLogT>> for Deposit<LogT> {
    fn map_logs(self, map_fn: impl FnMut(LogT) -> NewLogT) -> Deposit<NewLogT> {
        Deposit {
            status: self.status,
            cumulative_gas_used: self.cumulative_gas_used,
            logs_bloom: self.logs_bloom,
            logs: self.logs.into_iter().map(map_fn).collect(),
            transaction_type: self.transaction_type,
            deposit_nonce: self.deposit_nonce,
            deposit_receipt_version: self.deposit_receipt_version,
        }
    }
}
