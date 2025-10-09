use core::fmt::Debug;

use edr_receipt::{log::FilterLog, ExecutionReceipt, ReceiptTrait};

pub trait ChainReceiptSpec {
    /// Type representing a transaction's receipt in a block.
    type TransactionReceipt: Debug + ExecutionReceipt<Log = FilterLog> + ReceiptTrait;
}
