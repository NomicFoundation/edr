//! Ethereum L1 receipt types

pub mod builder;

use std::ops::Deref;

use alloy_rlp::BufMut;
use edr_chain_spec::{
    ContextChainSpec, EvmSpecId, ProtocolHardfork as _, ProtocolHardforkChainSpec,
};
use edr_chain_spec_receipt::ReceiptConstructor;
use edr_primitives::{Address, Bloom, B256};
use edr_receipt::{
    log::FilterLog, AsExecutionReceipt, ExecutionReceipt, ReceiptTrait, RootOrStatus,
    TransactionReceipt,
};

use crate::L1ChainSpec;

/// Type for a receipt that's included in a block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct L1BlockReceipt<ExecutionReceiptT: ExecutionReceipt<Log = FilterLog>> {
    pub inner: TransactionReceipt<ExecutionReceiptT>,
    /// Hash of the block that this is part of
    pub block_hash: B256,
    /// Number of the block that this is part of
    pub block_number: u64,
}

impl<ExecutionReceiptT: ExecutionReceipt<Log = FilterLog>> L1BlockReceipt<ExecutionReceiptT> {
    /// Constructs a new instance from a transaction's receipt and the block it
    /// was executed in.
    pub fn new(
        mut inner: TransactionReceipt<ExecutionReceiptT>,
        evm_hardfork: EvmSpecId,
        block_hash: B256,
        block_number: u64,
    ) -> Self {
        // The JSON-RPC layer should not return the gas price as effective gas
        // price for receipts in hardforks that predate EIP-1559 (London).
        if evm_hardfork < EvmSpecId::LONDON {
            inner.effective_gas_price = None;
        }

        Self {
            inner,
            block_hash,
            block_number,
        }
    }
}

impl<ExecutionReceiptT: ExecutionReceipt<Log = FilterLog>> AsExecutionReceipt
    for L1BlockReceipt<ExecutionReceiptT>
{
    type ExecutionReceipt = ExecutionReceiptT;

    fn as_execution_receipt(&self) -> &ExecutionReceiptT {
        self.inner.as_execution_receipt()
    }
}

impl<ExecutionReceiptT: ExecutionReceipt<Log = FilterLog>> Deref
    for L1BlockReceipt<ExecutionReceiptT>
{
    type Target = TransactionReceipt<ExecutionReceiptT>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<ExecutionReceiptT> alloy_rlp::Encodable for L1BlockReceipt<ExecutionReceiptT>
where
    ExecutionReceiptT: ExecutionReceipt<Log = FilterLog> + alloy_rlp::Encodable,
{
    fn encode(&self, out: &mut dyn BufMut) {
        self.inner.encode(out);
    }

    fn length(&self) -> usize {
        self.inner.length()
    }
}

impl<ExecutionReceiptT: ExecutionReceipt<Log = FilterLog>> ExecutionReceipt
    for L1BlockReceipt<ExecutionReceiptT>
{
    type Log = ExecutionReceiptT::Log;

    fn cumulative_gas_used(&self) -> u64 {
        self.inner.cumulative_gas_used()
    }

    fn logs_bloom(&self) -> &Bloom {
        self.inner.logs_bloom()
    }

    fn transaction_logs(&self) -> &[Self::Log] {
        self.inner.transaction_logs()
    }

    fn root_or_status(&self) -> RootOrStatus<'_> {
        self.inner.root_or_status()
    }
}

impl<ExecutionReceiptT: ExecutionReceipt<Log = FilterLog>, SignedTransactionT>
    ReceiptConstructor<SignedTransactionT> for L1BlockReceipt<ExecutionReceiptT>
{
    type Context = <L1ChainSpec as ContextChainSpec>::Context;

    type ExecutionReceipt = ExecutionReceiptT;

    type Hardfork = <L1ChainSpec as ProtocolHardforkChainSpec>::ProtocolHardfork;

    fn new_receipt(
        _context: &Self::Context,
        hardfork: Self::Hardfork,
        _transaction: &SignedTransactionT,
        transaction_receipt: TransactionReceipt<Self::ExecutionReceipt>,
        block_hash: &B256,
        block_number: u64,
    ) -> Self {
        L1BlockReceipt::new(
            transaction_receipt,
            hardfork.to_evm_spec_id(),
            *block_hash,
            block_number,
        )
    }
}

impl<ExecutionReceiptT> ReceiptTrait for L1BlockReceipt<ExecutionReceiptT>
where
    ExecutionReceiptT: ExecutionReceipt<Log = FilterLog>,
{
    fn block_number(&self) -> u64 {
        self.block_number
    }

    fn block_hash(&self) -> &B256 {
        &self.block_hash
    }

    fn contract_address(&self) -> Option<&Address> {
        self.inner.contract_address.as_ref()
    }

    fn effective_gas_price(&self) -> Option<&u128> {
        self.inner.effective_gas_price.as_ref()
    }

    fn from(&self) -> &Address {
        &self.inner.from
    }

    fn gas_used(&self) -> u64 {
        self.inner.gas_used
    }

    fn to(&self) -> Option<&Address> {
        self.inner.to.as_ref()
    }

    fn transaction_hash(&self) -> &B256 {
        &self.inner.transaction_hash
    }

    fn transaction_index(&self) -> u64 {
        self.inner.transaction_index
    }
}
