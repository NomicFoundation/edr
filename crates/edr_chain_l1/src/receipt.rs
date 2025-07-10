pub use edr_eth::receipt::*;
use edr_eth::{
    log::{logs_to_bloom, ExecutionLog},
    result::ExecutionResult,
    transaction::TransactionType as _,
};
use edr_evm::{receipt::ExecutionReceiptBuilder, state::State};

use crate::{block, eip2718::TypedEnvelope, transaction, HaltReason, Hardfork};

/// Convenience type alias for [`L1ReceiptBuilder`].
///
/// This allows usage like [`edr_chain_l1::receipt::Builder`].
pub type Builder = L1ReceiptBuilder;

/// Builder for execution receipts.
pub struct L1ReceiptBuilder;

impl ExecutionReceiptBuilder<HaltReason, Hardfork, transaction::Signed> for L1ReceiptBuilder {
    type Receipt = TypedEnvelope<edr_eth::receipt::execution::Eip658<ExecutionLog>>;

    fn new_receipt_builder<StateT: State>(
        _pre_execution_state: StateT,
        _transaction: &transaction::Signed,
    ) -> Result<Self, StateT::Error> {
        Ok(Self)
    }

    fn build_receipt(
        self,
        header: &block::PartialHeader,
        transaction: &transaction::Signed,
        result: &ExecutionResult<HaltReason>,
        _hardfork: Hardfork,
    ) -> Self::Receipt {
        let logs = result.logs().to_vec();
        let logs_bloom = logs_to_bloom(&logs);

        let receipt = execution::Eip658 {
            status: result.is_success(),
            cumulative_gas_used: header.gas_used,
            logs_bloom,
            logs,
        };

        TypedEnvelope::new(receipt, transaction.transaction_type())
    }
}
