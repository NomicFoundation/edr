use edr_chain_l1::{L1HaltReason, L1Hardfork};
use edr_eth::{log::ExecutionLog, receipt, result::ExecutionResult, transaction::TransactionType};
use edr_evm::{receipt::ExecutionReceiptBuilder, state::State};

use crate::{eip2718::TypedEnvelope, transaction};

pub struct Builder;

impl ExecutionReceiptBuilder<L1HaltReason, L1Hardfork, transaction::SignedWithFallbackToPostEip155>
    for Builder
{
    type Receipt = TypedEnvelope<receipt::execution::Eip658<ExecutionLog>>;

    fn new_receipt_builder<StateT: State>(
        _pre_execution_state: StateT,
        _transaction: &transaction::SignedWithFallbackToPostEip155,
    ) -> Result<Self, StateT::Error> {
        Ok(Self)
    }

    fn build_receipt(
        self,
        header: &edr_eth::block::PartialHeader,
        transaction: &crate::transaction::SignedWithFallbackToPostEip155,
        result: &ExecutionResult<L1HaltReason>,
        _hardfork: L1Hardfork,
    ) -> Self::Receipt {
        let logs = result.logs().to_vec();
        let logs_bloom = edr_eth::log::logs_to_bloom(&logs);

        let receipt = receipt::execution::Eip658 {
            status: result.is_success(),
            cumulative_gas_used: header.gas_used,
            logs_bloom,
            logs,
        };

        TypedEnvelope::new(receipt, transaction.transaction_type())
    }
}
