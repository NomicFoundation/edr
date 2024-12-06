use std::ops::Deref;

use alloy_rlp::BufMut;

use super::{ExecutionReceipt, ReceiptTrait, RootOrStatus, TransactionReceipt};
use crate::{log::FilterLog, Bloom, B256};

/// Type for a receipt that's included in a block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockReceipt<ExecutionReceiptT: ExecutionReceipt<Log = FilterLog>> {
    pub inner: TransactionReceipt<ExecutionReceiptT>,
    /// Hash of the block that this is part of
    pub block_hash: B256,
    /// Number of the block that this is part of
    pub block_number: u64,
}

impl<ExecutionReceiptT: ExecutionReceipt<Log = FilterLog>> Deref
    for BlockReceipt<ExecutionReceiptT>
{
    type Target = TransactionReceipt<ExecutionReceiptT>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<ExecutionReceiptT> alloy_rlp::Encodable for BlockReceipt<ExecutionReceiptT>
where
    ExecutionReceiptT: ExecutionReceipt<Log = FilterLog> + alloy_rlp::Encodable,
{
    fn encode(&self, out: &mut dyn BufMut) {
        self.inner.encode(out);
    }

    fn length(&self) -> usize {
        self.inner.length()
    }
}

impl<ExecutionReceiptT: ExecutionReceipt<Log = FilterLog>> ExecutionReceipt
    for BlockReceipt<ExecutionReceiptT>
{
    type Log = ExecutionReceiptT::Log;

    fn cumulative_gas_used(&self) -> u64 {
        self.inner.cumulative_gas_used()
    }

    fn logs_bloom(&self) -> &Bloom {
        self.inner.logs_bloom()
    }

    fn transaction_logs(&self) -> &[Self::Log] {
        self.inner.transaction_logs()
    }

    fn root_or_status(&self) -> RootOrStatus<'_> {
        self.inner.root_or_status()
    }
}

impl<ExecutionReceiptT> ReceiptTrait for BlockReceipt<ExecutionReceiptT>
where
    ExecutionReceiptT: ExecutionReceipt<Log = FilterLog>,
{
    fn transaction_hash(&self) -> &B256 {
        &self.inner.transaction_hash
    }
}
