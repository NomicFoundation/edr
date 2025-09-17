//! Types for execution receipts.

mod eip658;
mod legacy;

use alloy_rlp::{RlpDecodable, RlpEncodable};
use edr_primitives::{Bloom, B256};

use super::{Execution, ExecutionReceipt, MapReceiptLogs};

#[derive(
    Clone, Debug, PartialEq, Eq, RlpDecodable, serde::Deserialize, serde::Serialize, RlpEncodable,
)]
#[serde(rename_all = "camelCase")]
pub struct Legacy<LogT> {
    /// State root
    pub root: B256,
    /// Cumulative gas used in block after this transaction was executed
    #[serde(with = "alloy_serde::quantity")]
    pub cumulative_gas_used: u64,
    /// Bloom filter of the logs generated within this transaction
    pub logs_bloom: Bloom,
    /// Logs generated within this transaction
    pub logs: Vec<LogT>,
}

#[derive(
    Clone, Debug, PartialEq, Eq, RlpDecodable, RlpEncodable, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "camelCase")]
pub struct Eip658<LogT> {
    /// Status
    #[serde(with = "alloy_serde::quantity")]
    pub status: bool,
    /// Cumulative gas used in block after this transaction was executed
    #[serde(with = "alloy_serde::quantity")]
    pub cumulative_gas_used: u64,
    /// Bloom filter of the logs generated within this transaction
    pub logs_bloom: Bloom,
    /// Logs generated within this transaction
    pub logs: Vec<LogT>,
}

impl<LogT> From<Legacy<LogT>> for Eip658<LogT> {
    fn from(value: Legacy<LogT>) -> Self {
        Self {
            // Execution would never revert before EIP-658, so was always considered successful
            status: true,
            cumulative_gas_used: value.cumulative_gas_used,
            logs_bloom: value.logs_bloom,
            logs: value.logs,
        }
    }
}

impl<LogT> From<Legacy<LogT>> for Execution<LogT> {
    fn from(value: Legacy<LogT>) -> Self {
        Execution::Legacy(value)
    }
}

impl<LogT> From<Eip658<LogT>> for Execution<LogT> {
    fn from(value: Eip658<LogT>) -> Self {
        Execution::Eip658(value)
    }
}

impl<LogT, NewLogT> MapReceiptLogs<LogT, NewLogT, Execution<NewLogT>> for Execution<LogT> {
    fn map_logs(self, map_fn: impl FnMut(LogT) -> NewLogT) -> Execution<NewLogT> {
        match self {
            Execution::Legacy(receipt) => Execution::Legacy(receipt.map_logs(map_fn)),
            Execution::Eip658(receipt) => Execution::Eip658(receipt.map_logs(map_fn)),
        }
    }
}

impl<LogT> alloy_rlp::Decodable for Execution<LogT>
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
            let receipt = Eip658::<LogT>::decode(buf)?;
            Ok(Self::Eip658(receipt))
        } else {
            let receipt = Legacy::<LogT>::decode(buf)?;
            Ok(Self::Legacy(receipt))
        }
    }
}

impl<LogT> alloy_rlp::Encodable for Execution<LogT>
where
    LogT: alloy_rlp::Encodable,
{
    fn encode(&self, out: &mut dyn alloy_rlp::BufMut) {
        match self {
            Execution::Legacy(receipt) => receipt.encode(out),
            Execution::Eip658(receipt) => receipt.encode(out),
        }
    }

    fn length(&self) -> usize {
        match self {
            Execution::Legacy(receipt) => receipt.length(),
            Execution::Eip658(receipt) => receipt.length(),
        }
    }
}

impl<LogT> ExecutionReceipt for Execution<LogT> {
    type Log = LogT;

    fn cumulative_gas_used(&self) -> u64 {
        match self {
            Execution::Legacy(receipt) => receipt.cumulative_gas_used,
            Execution::Eip658(receipt) => receipt.cumulative_gas_used,
        }
    }

    fn logs_bloom(&self) -> &Bloom {
        match self {
            Execution::Legacy(receipt) => &receipt.logs_bloom,
            Execution::Eip658(receipt) => &receipt.logs_bloom,
        }
    }

    fn transaction_logs(&self) -> &[LogT] {
        match self {
            Execution::Legacy(receipt) => &receipt.logs,
            Execution::Eip658(receipt) => &receipt.logs,
        }
    }

    fn root_or_status(&self) -> super::RootOrStatus<'_> {
        match self {
            Execution::Legacy(receipt) => super::RootOrStatus::Root(&receipt.root),
            Execution::Eip658(receipt) => super::RootOrStatus::Status(receipt.status),
        }
    }
}
