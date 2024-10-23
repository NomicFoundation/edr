use std::ops::Deref;

use alloy_rlp::BufMut;

use super::{Receipt, TransactionReceipt};
use crate::{log::FilterLog, B256};

/// Type for a receipt that's included in a block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockReceipt<ExecutionReceiptT: Receipt<FilterLog>> {
    pub inner: TransactionReceipt<ExecutionReceiptT, FilterLog>,
    /// Hash of the block that this is part of
    pub block_hash: B256,
    /// Number of the block that this is part of
    pub block_number: u64,
}

impl<ExecutionReceiptT: Receipt<FilterLog>> Deref for BlockReceipt<ExecutionReceiptT> {
    type Target = TransactionReceipt<ExecutionReceiptT, FilterLog>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<ExecutionReceiptT> alloy_rlp::Encodable for BlockReceipt<ExecutionReceiptT>
where
    ExecutionReceiptT: Receipt<FilterLog> + alloy_rlp::Encodable,
{
    fn encode(&self, out: &mut dyn BufMut) {
        self.inner.encode(out);
    }

    fn length(&self) -> usize {
        self.inner.length()
    }
}
