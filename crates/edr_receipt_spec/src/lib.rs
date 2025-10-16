use core::fmt::Debug;

use edr_chain_spec::{ChainContextSpec, ChainHardfork, ChainSpec};
use edr_primitives::B256;
use edr_receipt::{
    log::FilterLog, ExecutionReceiptChainSpec, ExecutionReceipt, ReceiptTrait, TransactionReceipt,
};
use edr_rpc_spec::RpcSpec;

/// Trait for a chain's transaction receipt specification.
pub trait ChainReceiptSpec:
    ChainContextSpec + ExecutionReceiptChainSpec + ChainHardfork + ChainSpec + RpcSpec
{
    /// Type representing a transaction's receipt in a block.
    type Receipt: Debug
        + ExecutionReceipt<Log = FilterLog>
        + ReceiptConstructor<
            Context = Self::Context,
            ExecutionReceipt = Self::ExecutionReceipt<FilterLog>,
            Hardfork = Self::Hardfork,
            SignedTransaction = Self::SignedTransaction,
        > + ReceiptTrait
        + TryFrom<Self::RpcReceipt>;
}

/// Trait for constructing a receipt type from a transaction's execution receipt
/// and the block it was executed in.
pub trait ReceiptConstructor {
    /// Type representing the receipt's contextual information.
    type Context;

    /// Type representing an execution receipt.
    type ExecutionReceipt: ExecutionReceipt<Log = FilterLog>;

    /// Type representing the receipt's hardfork type.
    type Hardfork;

    /// Type representing a signed transaction.
    type SignedTransaction;

    /// Constructs a new instance from a transaction's receipt and the block it
    /// was executed in.
    fn new_receipt(
        context: &Self::Context,
        hardfork: Self::Hardfork,
        transaction: &Self::SignedTransaction,
        transaction_receipt: TransactionReceipt<Self::ExecutionReceipt>,
        block_hash: &B256,
        block_number: u64,
    ) -> Self;
}
