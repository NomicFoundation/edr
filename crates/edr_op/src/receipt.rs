mod block;
/// Types for OP execution receipts.
pub mod execution;
mod factory;

pub use edr_evm::receipt::ExecutionReceiptBuilder;

pub use self::{block::Block, factory::BlockReceiptFactory};

/// OP execution receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Execution<LogT> {
    /// EIP-658 receipt.
    Eip658(edr_eth::receipt::execution::Eip658<LogT>),
    /// OP deposit receipt (post-Regolith).
    Deposit(self::execution::Deposit<LogT>),
}
