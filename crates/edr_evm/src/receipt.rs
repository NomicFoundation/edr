use edr_block_header::PartialHeader;
use edr_chain_l1::TypedEnvelope;
use edr_evm_spec::{ExecutableTransaction, HaltReasonTrait, TransactionValidation};
use edr_receipt::log::{logs_to_bloom, ExecutionLog};
use edr_state_api::State;
use edr_transaction::TransactionType as _;

use crate::result::ExecutionResult;

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
        header: &PartialHeader,
        transaction: &TransactionT,
        result: &ExecutionResult<HaltReasonT>,
        hardfork: HardforkT,
    ) -> Self::Receipt;
}

/// Builder for execution receipts.
pub struct Builder;

impl
    ExecutionReceiptBuilder<
        edr_chain_l1::HaltReason,
        edr_chain_l1::Hardfork,
        edr_chain_l1::L1SignedTransaction,
    > for Builder
{
    type Receipt = TypedEnvelope<edr_receipt::execution::Eip658<ExecutionLog>>;

    fn new_receipt_builder<StateT: State>(
        _pre_execution_state: StateT,
        _transaction: &edr_chain_l1::L1SignedTransaction,
    ) -> Result<Self, StateT::Error> {
        Ok(Self)
    }

    fn build_receipt(
        self,
        header: &PartialHeader,
        transaction: &edr_chain_l1::L1SignedTransaction,
        result: &ExecutionResult<edr_chain_l1::HaltReason>,
        _hardfork: edr_chain_l1::Hardfork,
    ) -> Self::Receipt {
        let logs = result.logs().to_vec();
        let logs_bloom = logs_to_bloom(&logs);

        let receipt = edr_receipt::execution::Eip658 {
            status: result.is_success(),
            cumulative_gas_used: header.gas_used,
            logs_bloom,
            logs,
        };

        TypedEnvelope::new(receipt, transaction.transaction_type())
    }
}
