use alloy_rlp::{RlpDecodable, RlpEncodable};
use edr_primitives::Bloom;
use edr_receipt::MapReceiptLogs;

use super::OpExecutionReceipt;

/// Receipt for an OP deposit transaction with deposit nonce (since
/// Regolith) and optionally deposit receipt version (since Canyon).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deposit<LogT> {
    /// Status
    pub status: bool,
    /// Cumulative gas used in block after this transaction was executed
    pub cumulative_gas_used: u64,
    /// Bloom filter of the logs generated within this transaction
    pub logs_bloom: Bloom,
    /// Logs generated within this transaction
    pub logs: Vec<LogT>,
    /// The nonce used during execution.
    pub deposit_nonce: u64,
    /// The deposit receipt version.
    ///
    /// The deposit receipt version was introduced in Canyon to indicate an
    /// update to how receipt hashes should be computed when set. The state
    /// transition process ensures this is only set for post-Canyon deposit
    /// transactions.
    pub deposit_receipt_version: Option<u8>,
}

#[derive(RlpDecodable)]
#[rlp(trailing)]
pub(super) struct Eip658OrDeposit<LogT> {
    status: bool,
    cumulative_gas_used: u64,
    logs_bloom: Bloom,
    logs: Vec<LogT>,
    deposit_nonce: Option<u64>,
    deposit_receipt_version: Option<u8>,
}

impl<LogT> From<Eip658OrDeposit<LogT>> for OpExecutionReceipt<LogT> {
    fn from(value: Eip658OrDeposit<LogT>) -> Self {
        if let Some(deposit_nonce) = value.deposit_nonce {
            OpExecutionReceipt::Deposit(Deposit {
                status: value.status,
                cumulative_gas_used: value.cumulative_gas_used,
                logs_bloom: value.logs_bloom,
                logs: value.logs,
                deposit_nonce,
                deposit_receipt_version: value.deposit_receipt_version,
            })
        } else {
            OpExecutionReceipt::Eip658(super::Eip658 {
                status: value.status,
                cumulative_gas_used: value.cumulative_gas_used,
                logs_bloom: value.logs_bloom,
                logs: value.logs,
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
        Encodable::from(self).encode(out);
    }

    fn length(&self) -> usize {
        Encodable::from(self).length()
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
            deposit_nonce: self.deposit_nonce,
            deposit_receipt_version: self.deposit_receipt_version,
        }
    }
}
