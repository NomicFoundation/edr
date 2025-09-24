use edr_block_header::PartialHeader;
use edr_evm::{receipt::ExecutionReceiptBuilder, result::ExecutionResult, state::State};
use edr_receipt::log::{logs_to_bloom, ExecutionLog};
use edr_transaction::TransactionType;

use crate::{eip2718::TypedEnvelope, transaction};

pub struct Builder;

impl
    ExecutionReceiptBuilder<
        edr_chain_l1::HaltReason,
        edr_chain_l1::Hardfork,
        transaction::SignedWithFallbackToPostEip155,
    > for Builder
{
    type Receipt = TypedEnvelope<edr_receipt::execution::Eip658<ExecutionLog>>;

    fn new_receipt_builder<StateT: State>(
        _pre_execution_state: StateT,
        _transaction: &transaction::SignedWithFallbackToPostEip155,
    ) -> Result<Self, StateT::Error> {
        Ok(Self)
    }

    fn build_receipt(
        self,
        header: &PartialHeader,
        transaction: &crate::transaction::SignedWithFallbackToPostEip155,
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
