// Re-export the receipt types from `edr_eth`.
pub use edr_eth::receipt::*;
use edr_eth::{
    block,
    result::ExecutionResult,
    spec::HaltReasonTrait,
    transaction::{ExecutableTransaction, TransactionValidation},
};

use crate::state::State;

/// Trait for a builder that constructs an execution receipt.
pub trait ExecutionReceiptBuilder<HaltReasonT, HardforkT, TransactionT>: Sized
where
    HaltReasonT: HaltReasonTrait,
    TransactionT: ExecutableTransaction + TransactionValidation,
{
    /// The receipt type that the builder constructs.
    type Receipt;

    /// Creates a new builder with the given pre-execution state.
    fn new_receipt_builder<StateT: State>(
        pre_execution_state: StateT,
        transaction: &TransactionT,
    ) -> Result<Self, StateT::Error>;

    /// Builds a receipt using the provided information.
    fn build_receipt(
        self,
        header: &block::PartialHeader,
        transaction: &TransactionT,
        result: &ExecutionResult<HaltReasonT>,
        hardfork: HardforkT,
    ) -> Self::Receipt;
}
