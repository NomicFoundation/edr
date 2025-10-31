//! Types needed to implement a builder pattern for execution receipts.

use edr_block_header::PartialHeader;
use edr_evm_spec::result::ExecutionResult;
use edr_state_api::State;

/// Trait for a builder that constructs an execution receipt.
pub trait ExecutionReceiptBuilder<HaltReasonT, HardforkT, SignedTransactionT>: Sized {
    /// The receipt type that the builder constructs.
    type Receipt;

    /// Creates a new builder with the given pre-execution state.
    fn new_receipt_builder<StateT: State>(
        pre_execution_state: StateT,
        transaction: &SignedTransactionT,
    ) -> Result<Self, StateT::Error>;

    /// Builds a receipt using the provided information.
    fn build_receipt(
        self,
        header: &PartialHeader,
        transaction: &SignedTransactionT,
        result: &ExecutionResult<HaltReasonT>,
        hardfork: HardforkT,
    ) -> Self::Receipt;
}
