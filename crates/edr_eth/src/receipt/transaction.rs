use std::marker::PhantomData;

use alloy_rlp::BufMut;
use revm_primitives::{ChainSpec, ExecutionResult, Output};

use super::{MapReceiptLogs, Receipt};
use crate::{
    transaction::{SignedTransaction, TransactionType},
    Address, Bloom, SpecId, B256, U256,
};

/// Type for a receipt that's created when processing a transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransactionReceipt<ExecutionReceiptT, LogT> {
    pub inner: ExecutionReceiptT,
    /// Hash of the transaction
    pub transaction_hash: B256,
    /// Index of the transaction in the block
    pub transaction_index: u64,
    /// Address of the sender
    pub from: Address,
    /// Address of the receiver. `None` when it's a contract creation
    /// transaction.
    pub to: Option<Address>,
    /// The contract address created, if the transaction was a contract
    /// creation, otherwise `None`.
    pub contract_address: Option<Address>,
    /// Gas used by this transaction alone.
    pub gas_used: u64,
    /// The actual value per gas deducted from the senders account, which is
    /// equal to baseFeePerGas + min(maxFeePerGas - baseFeePerGas,
    /// maxPriorityFeePerGas) after EIP-1559. Following Hardhat, only present if
    /// the hardfork is at least London.
    pub effective_gas_price: Option<U256>,
    pub phantom: PhantomData<LogT>,
}

impl<ExecutionReceiptT: Receipt<LogT>, LogT> TransactionReceipt<ExecutionReceiptT, LogT> {
    /// Returns a reference to the inner execution receipt.
    pub fn as_execution_receipt(&self) -> &ExecutionReceiptT {
        &self.inner
    }

    /// Converts the instance into the inner execution receipt.
    pub fn into_execution_receipt(self) -> ExecutionReceiptT {
        self.inner
    }
}

impl<ExecutionReceiptT: Receipt<LogT>, LogT> TransactionReceipt<ExecutionReceiptT, LogT> {
    /// Constructs a new instance using the provided execution receipt an
    /// transaction
    pub fn new<ChainSpecT>(
        execution_receipt: ExecutionReceiptT,
        transaction: &impl SignedTransaction,
        result: &ExecutionResult<ChainSpecT>,
        transaction_index: u64,
        block_base_fee: U256,
        hardfork: ChainSpecT::Hardfork,
    ) -> Self
    where
        ChainSpecT: ChainSpec,
    {
        let contract_address = if let ExecutionResult::Success {
            output: Output::Create(_, address),
            ..
        } = result
        {
            *address
        } else {
            None
        };

        let effective_gas_price = if hardfork.into() >= SpecId::LONDON {
            Some(
                transaction
                    .effective_gas_price(block_base_fee)
                    .unwrap_or_else(|| *transaction.gas_price()),
            )
        } else {
            None
        };

        Self {
            inner: execution_receipt,
            transaction_hash: *transaction.transaction_hash(),
            transaction_index,
            from: *transaction.caller(),
            to: transaction.kind().to().copied(),
            contract_address,
            gas_used: result.gas_used(),
            effective_gas_price,
            phantom: PhantomData,
        }
    }
}

impl<OldExecutionReceiptT, NewExecutionReceiptT, OldLogT, NewLogT>
    MapReceiptLogs<OldLogT, NewLogT, TransactionReceipt<NewExecutionReceiptT, NewLogT>>
    for TransactionReceipt<OldExecutionReceiptT, OldLogT>
where
    OldExecutionReceiptT: MapReceiptLogs<OldLogT, NewLogT, NewExecutionReceiptT> + Receipt<OldLogT>,
    NewExecutionReceiptT: Receipt<NewLogT>,
{
    fn map_logs(
        self,
        map_fn: impl FnMut(OldLogT) -> NewLogT,
    ) -> TransactionReceipt<NewExecutionReceiptT, NewLogT> {
        TransactionReceipt {
            inner: self.inner.map_logs(map_fn),
            transaction_hash: self.transaction_hash,
            transaction_index: self.transaction_index,
            from: self.from,
            to: self.to,
            contract_address: self.contract_address,
            gas_used: self.gas_used,
            effective_gas_price: self.effective_gas_price,
            phantom: PhantomData,
        }
    }
}

impl<ExecutionReceiptT: Receipt<LogT>, LogT> Receipt<LogT>
    for TransactionReceipt<ExecutionReceiptT, LogT>
{
    fn cumulative_gas_used(&self) -> u64 {
        self.inner.cumulative_gas_used()
    }

    fn logs_bloom(&self) -> &Bloom {
        self.inner.logs_bloom()
    }

    fn logs(&self) -> &[LogT] {
        self.inner.logs()
    }

    fn root_or_status(&self) -> super::RootOrStatus<'_> {
        self.inner.root_or_status()
    }
}

impl<ExecutionReceiptT: TransactionType, LogT> TransactionType
    for TransactionReceipt<ExecutionReceiptT, LogT>
{
    type Type = ExecutionReceiptT::Type;

    fn transaction_type(&self) -> Self::Type {
        self.inner.transaction_type()
    }
}

impl<ExecutionReceiptT, LogT> alloy_rlp::Encodable for TransactionReceipt<ExecutionReceiptT, LogT>
where
    ExecutionReceiptT: Receipt<LogT> + alloy_rlp::Encodable,
{
    fn encode(&self, out: &mut dyn BufMut) {
        self.inner.encode(out);
    }

    fn length(&self) -> usize {
        self.inner.length()
    }
}
