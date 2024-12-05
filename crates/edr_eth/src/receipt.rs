// Part of this code was adapted from foundry and is distributed under their
// licenss:
// - https://github.com/foundry-rs/foundry/blob/01b16238ff87dc7ca8ee3f5f13e389888c2a2ee4/LICENSE-APACHE
// - https://github.com/foundry-rs/foundry/blob/01b16238ff87dc7ca8ee3f5f13e389888c2a2ee4/LICENSE-MIT
// For the original context see: https://github.com/foundry-rs/foundry/blob/01b16238ff87dc7ca8ee3f5f13e389888c2a2ee4/anvil/core/src/eth/receipt.rs

#![allow(missing_docs)]

mod block;
/// Types for execution receipts.
pub mod execution;
mod transaction;

pub use self::{block::BlockReceipt, transaction::TransactionReceipt};
use crate::{Bloom, B256};

/// Log generated after execution of a transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize), serde(untagged))]
pub enum Execution<LogT> {
    /// Legacy receipt.
    Legacy(self::execution::Legacy<LogT>),
    /// EIP-658 receipt.
    Eip658(self::execution::Eip658<LogT>),
}

/// Type representing either the state root (pre-EIP-658) or the status code
/// (post-EIP-658).
#[derive(Debug, PartialEq, Eq)]
pub enum RootOrStatus<'root> {
    /// State root (pre-EIP-658).
    Root(&'root B256),
    /// Status code (post-EIP-658).
    Status(bool),
}

/// Trait for a receipt that's generated after execution of a transaction.
pub trait ExecutionReceipt<LogT> {
    /// Returns the cumulative gas used in the block after this transaction was
    /// executed.
    fn cumulative_gas_used(&self) -> u64;
    /// Returns the bloom filter of the logs generated within this transaction.
    fn logs_bloom(&self) -> &Bloom;
    /// Returns the logs generated within this transaction.
    fn transaction_logs(&self) -> &[LogT];
    /// Returns the state root (pre-EIP-658) or status (post-EIP-658) of the
    /// receipt.
    fn root_or_status(&self) -> RootOrStatus<'_>;
}

pub trait MapReceiptLogs<OldLogT, NewLogT, OutputT> {
    /// Maps the logs of the receipt to a new type.
    fn map_logs(self, map_fn: impl FnMut(OldLogT) -> NewLogT) -> OutputT;
}
