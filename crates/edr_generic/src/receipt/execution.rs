use edr_eth::{
    log::ExecutionLog,
    receipt::{
        execution::{Eip658, Legacy},
        Execution, ExecutionReceiptBuilder,
    },
    transaction::TransactionType,
};
use revm_primitives::{EvmWiring, SpecId};

use crate::{eip2718::TypedEnvelope, GenericChainSpec};

pub struct Builder;

impl ExecutionReceiptBuilder<GenericChainSpec> for Builder {
    type Receipt = TypedEnvelope<Execution<ExecutionLog>>;

    fn new_receipt_builder<StateT: revm::db::StateRef>(
        _pre_execution_state: StateT,
        _transaction: &<GenericChainSpec as EvmWiring>::Transaction,
    ) -> Result<Self, StateT::Error> {
        Ok(Self)
    }

    fn build_receipt(
        self,
        header: &edr_eth::block::PartialHeader,
        transaction: &crate::transaction::SignedFallbackToPostEip155,
        result: &revm_primitives::ExecutionResult<GenericChainSpec>,
        hardfork: SpecId,
    ) -> Self::Receipt {
        let logs = result.logs().to_vec();
        let logs_bloom = edr_eth::log::logs_to_bloom(&logs);

        let receipt = if hardfork >= SpecId::BYZANTIUM {
            Execution::Eip658(Eip658 {
                status: result.is_success(),
                cumulative_gas_used: header.gas_used,
                logs_bloom,
                logs,
            })
        } else {
            Execution::Legacy(Legacy {
                root: header.state_root,
                cumulative_gas_used: header.gas_used,
                logs_bloom,
                logs,
            })
        };

        TypedEnvelope::new(receipt, transaction.transaction_type())
    }
}
