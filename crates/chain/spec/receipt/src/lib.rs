use core::fmt::Debug;

use edr_chain_spec::{ChainSpec, ContextChainSpec, HardforkChainSpec};
use edr_chain_spec_rpc::RpcChainSpec;
use edr_primitives::B256;
use edr_receipt::{
    log::{ExecutionLog, FilterLog},
    ExecutionReceipt, ExecutionReceiptChainSpec, ReceiptTrait, TransactionReceipt,
};
use edr_receipt_builder_api::ExecutionReceiptBuilder;

/// Trait for a chain's transaction receipt specification.
pub trait ReceiptChainSpec:
    ContextChainSpec + ExecutionReceiptChainSpec + HardforkChainSpec + ChainSpec + RpcChainSpec
{
    type ExecutionReceiptBuilder: ExecutionReceiptBuilder<
        Self::HaltReason,
        Self::Hardfork,
        Self::SignedTransaction,
        Receipt = Self::ExecutionReceipt<ExecutionLog>,
    >;

    /// Type representing a transaction's receipt in a block.
    type Receipt: Debug
        + ExecutionReceipt<Log = FilterLog>
        + ReceiptConstructor<
            Self::SignedTransaction,
            Context = Self::Context,
            ExecutionReceipt = Self::ExecutionReceipt<FilterLog>,
            Hardfork = Self::Hardfork,
        > + ReceiptTrait
        + TryFrom<Self::RpcReceipt, Error: std::error::Error>;
}

/// Trait for constructing a receipt type from a transaction's execution receipt
/// and the block it was executed in.
pub trait ReceiptConstructor<SignedTransactionT> {
    /// Type representing the receipt's contextual information.
    type Context;

    /// Type representing an execution receipt.
    type ExecutionReceipt: ExecutionReceipt<Log = FilterLog>;

    /// Type representing the receipt's hardfork type.
    type Hardfork;

    /// Constructs a new instance from a transaction's receipt and the block it
    /// was executed in.
    fn new_receipt(
        context: &Self::Context,
        hardfork: Self::Hardfork,
        transaction: &SignedTransactionT,
        transaction_receipt: TransactionReceipt<Self::ExecutionReceipt>,
        block_hash: &B256,
        block_number: u64,
    ) -> Self;
}
