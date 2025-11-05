//! Types for OP block receipts.

use edr_chain_l1::receipt::L1BlockReceipt;
use edr_chain_spec::ContextChainSpec;
use edr_primitives::{Address, Bloom, B256};
use edr_receipt::{
    log::FilterLog, AsExecutionReceipt, ExecutionReceipt, ReceiptTrait, RootOrStatus,
    TransactionReceipt,
};
use edr_receipt_spec::ReceiptConstructor;
use op_alloy_rpc_types::L1BlockInfo;
use op_revm::transaction::OpTxTr as _;

use crate::{
    eip2718::TypedEnvelope, receipt::execution::OpExecutionReceipt,
    transaction::signed::OpSignedTransaction, Hardfork, OpChainSpec,
};

/// An OP block receipt.
///
/// Includes the L1 block info for non-deposit transactions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpBlockReceipt {
    /// The underlying Ethereum block receipt.
    pub eth: L1BlockReceipt<TypedEnvelope<OpExecutionReceipt<FilterLog>>>,
    /// The L1 block info, if not a deposit transaction.
    pub l1_block_info: Option<L1BlockInfo>,
}

impl AsExecutionReceipt for OpBlockReceipt {
    type ExecutionReceipt = TypedEnvelope<OpExecutionReceipt<FilterLog>>;

    fn as_execution_receipt(&self) -> &Self::ExecutionReceipt {
        self.eth.as_execution_receipt()
    }
}

impl alloy_rlp::Encodable for OpBlockReceipt {
    fn encode(&self, out: &mut dyn alloy_rlp::BufMut) {
        self.eth.encode(out);
    }

    fn length(&self) -> usize {
        self.eth.length()
    }
}

impl ExecutionReceipt for OpBlockReceipt {
    type Log = FilterLog;

    fn cumulative_gas_used(&self) -> u64 {
        self.eth.cumulative_gas_used()
    }

    fn logs_bloom(&self) -> &Bloom {
        self.eth.logs_bloom()
    }

    fn transaction_logs(&self) -> &[Self::Log] {
        self.eth.transaction_logs()
    }

    fn root_or_status(&self) -> RootOrStatus<'_> {
        self.eth.root_or_status()
    }
}

impl ReceiptConstructor<OpSignedTransaction> for OpBlockReceipt {
    type Context = <OpChainSpec as ContextChainSpec>::Context;

    type ExecutionReceipt = TypedEnvelope<OpExecutionReceipt<FilterLog>>;

    type Hardfork = Hardfork;

    fn new_receipt(
        context: &Self::Context,
        hardfork: Self::Hardfork,
        transaction: &OpSignedTransaction,
        transaction_receipt: edr_receipt::TransactionReceipt<Self::ExecutionReceipt>,
        block_hash: &B256,
        block_number: u64,
    ) -> Self {
        let l1_block_info =
            to_rpc_l1_block_info(hardfork, context, transaction, &transaction_receipt);

        let eth = {
            L1BlockReceipt::new_receipt(
                &(),
                hardfork.into(),
                transaction,
                transaction_receipt,
                block_hash,
                block_number,
            )
        };

        Self { eth, l1_block_info }
    }
}

impl ReceiptTrait for OpBlockReceipt {
    fn block_number(&self) -> u64 {
        self.eth.block_number()
    }

    fn block_hash(&self) -> &B256 {
        self.eth.block_hash()
    }

    fn contract_address(&self) -> Option<&Address> {
        self.eth.contract_address()
    }

    fn effective_gas_price(&self) -> Option<&u128> {
        self.eth.effective_gas_price()
    }

    fn from(&self) -> &Address {
        self.eth.from()
    }

    fn gas_used(&self) -> u64 {
        self.eth.gas_used()
    }

    fn to(&self) -> Option<&Address> {
        self.eth.to()
    }

    fn transaction_hash(&self) -> &B256 {
        self.eth.transaction_hash()
    }

    fn transaction_index(&self) -> u64 {
        self.eth.transaction_index()
    }
}

fn to_rpc_l1_block_info(
    hardfork: Hardfork,
    l1_block_info: &op_revm::L1BlockInfo,
    transaction: &OpSignedTransaction,
    transaction_receipt: &TransactionReceipt<TypedEnvelope<OpExecutionReceipt<FilterLog>>>,
) -> Option<op_alloy_rpc_types::L1BlockInfo> {
    if matches!(
        transaction_receipt.inner,
        TypedEnvelope::Legacy(_) | TypedEnvelope::Deposit(_)
    ) {
        None
    } else {
        let enveloped_tx = transaction
            .enveloped_tx()
            .expect("Non-deposit transactions must return an enveloped transaction");

        let mut l1_block_info = l1_block_info.clone();
        let l1_fee = l1_block_info.calculate_tx_l1_cost(enveloped_tx, hardfork);

        let (l1_fee_scalar, l1_base_fee_scalar) = if hardfork < Hardfork::ECOTONE {
            let l1_fee_scalar: f64 = l1_block_info.l1_base_fee_scalar.into();

            (Some(l1_fee_scalar / 1_000_000f64), None)
        } else {
            let l1_base_fee_scalar = l1_block_info
                .l1_base_fee_scalar
                .try_into()
                .expect("L1 base fee scalar cannot be larger than u128::max");

            (None, Some(l1_base_fee_scalar))
        };

        let l1_gas_used = l1_block_info
            .data_gas(enveloped_tx, hardfork)
            .saturating_add(l1_block_info.l1_fee_overhead.unwrap_or_default());

        let l1_block_info = op_alloy_rpc_types::L1BlockInfo {
            l1_gas_price: Some(
                l1_block_info
                    .l1_base_fee
                    .try_into()
                    .expect("L1 gas price cannot be larger than u128::max"),
            ),
            l1_gas_used: Some(
                l1_gas_used
                    .try_into()
                    .expect("L1 gas used cannot be larger than u128::max"),
            ),
            l1_fee: Some(
                l1_fee
                    .try_into()
                    .expect("L1 fee cannot be larger than u128::max"),
            ),
            l1_fee_scalar,
            l1_base_fee_scalar,
            l1_blob_base_fee: l1_block_info.l1_blob_base_fee.map(|scalar| {
                scalar
                    .try_into()
                    .expect("L1 blob base fee cannot be larger than u128::max")
            }),
            l1_blob_base_fee_scalar: l1_block_info.l1_blob_base_fee_scalar.map(|scalar| {
                scalar
                    .try_into()
                    .expect("L1 blob base fee scalar cannot be larger than u128::max")
            }),
            operator_fee_scalar: l1_block_info.operator_fee_scalar.map(|scalar| {
                scalar
                    .try_into()
                    .expect("Operator fee scalar cannot be larger than u128::max")
            }),
            operator_fee_constant: l1_block_info.operator_fee_constant.map(|scalar| {
                scalar
                    .try_into()
                    .expect("Operator fee constant cannot be larger than u128::max")
            }),
        };

        Some(l1_block_info)
    }
}
