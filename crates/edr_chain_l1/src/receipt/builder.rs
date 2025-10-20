//! Ethereum L1 receipt builder

use edr_block_header::PartialHeader;
use edr_receipt::{
    log::{logs_to_bloom, ExecutionLog},
    ExecutionResult,
};
use edr_receipt_builder_api::ExecutionReceiptBuilder;
use edr_state_api::State;
use edr_transaction::TransactionType as _;

use crate::{HaltReason, Hardfork, L1SignedTransaction, TypedEnvelope};

/// Builder for execution receipts.
pub struct L1ExecutionReceiptBuilder;

impl ExecutionReceiptBuilder<HaltReason, Hardfork, L1SignedTransaction>
    for L1ExecutionReceiptBuilder
{
    type Receipt = TypedEnvelope<edr_receipt::execution::Eip658<ExecutionLog>>;

    fn new_receipt_builder<StateT: State>(
        _pre_execution_state: StateT,
        _transaction: &L1SignedTransaction,
    ) -> Result<Self, StateT::Error> {
        Ok(Self)
    }

    fn build_receipt(
        self,
        header: &PartialHeader,
        transaction: &L1SignedTransaction,
        result: &ExecutionResult<HaltReason>,
        _hardfork: Hardfork,
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
