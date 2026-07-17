use edr_chain_spec::EvmSpecId;
use edr_chain_spec_evm::result::ExecutionResult;
use edr_primitives::B256;
use edr_receipt::log::{logs_to_bloom, ExecutionLog};
use edr_receipt_builder_api::ExecutionReceiptBuilder;
use edr_state_api::State;
use edr_transaction::TransactionType;

use crate::{eip2718::TypedEnvelope, transaction};

pub struct GenericExecutionReceiptBuilder;

impl
    ExecutionReceiptBuilder<
        edr_chain_l1::HaltReason,
        edr_chain_l1::Hardfork,
        transaction::SignedTransactionWithFallbackToPostEip155,
    > for GenericExecutionReceiptBuilder
{
    type Receipt = TypedEnvelope<edr_receipt::Execution<ExecutionLog>>;

    fn new_receipt_builder<StateT: State>(
        _pre_execution_state: StateT,
        _transaction: &transaction::SignedTransactionWithFallbackToPostEip155,
    ) -> Result<Self, StateT::Error> {
        Ok(Self)
    }

    fn build_receipt(
        self,
        transaction: &crate::transaction::SignedTransactionWithFallbackToPostEip155,
        result: &ExecutionResult<edr_chain_l1::HaltReason>,
        hardfork: edr_chain_l1::Hardfork,
        cumulative_gas_used: u64,
        state_root: B256,
    ) -> Self::Receipt {
        let logs = result.logs().to_vec();
        let logs_bloom = logs_to_bloom(&logs);

        let receipt = if hardfork >= EvmSpecId::BYZANTIUM {
            edr_receipt::execution::Eip658 {
                status: result.is_success(),
                cumulative_gas_used,
                logs_bloom,
                logs,
            }
            .into()
        } else {
            edr_receipt::execution::Legacy {
                root: state_root,
                cumulative_gas_used,
                logs_bloom,
                logs,
            }
            .into()
        };

        TypedEnvelope::new(receipt, transaction.transaction_type())
    }
}
