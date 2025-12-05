use core::fmt::Debug;
use std::{marker::PhantomData, sync::Arc};

use edr_block_api::Block;
use edr_block_builder_api::BuiltBlockAndState;
use edr_chain_spec::{ChainSpec, ExecutableTransaction, HaltReasonTrait};
use edr_chain_spec_block::BlockChainSpec;
use edr_chain_spec_evm::result::ExecutionResult;
use edr_primitives::{Bytes, B256};
use edr_state_api::{DynState, StateDiff};
use foundry_evm_traces::SparsedTraceArena;

/// The result of mining a block, including the state, in debug mode. This
/// result needs to be inserted into the blockchain to be persistent.
pub struct DebugMineBlockResultAndState<HaltReasonT: HaltReasonTrait, LocalBlockT> {
    /// Mined block
    pub block: LocalBlockT,
    /// State after mining the block
    pub state: Box<dyn DynState>,
    /// State diff applied by block
    pub state_diff: StateDiff,
    /// Transaction results
    pub transaction_results: Vec<ExecutionResult<HaltReasonT>>,
    /// Transaction traces
    pub transaction_traces: Vec<SparsedTraceArena>,
    /// Encoded `console.log` call inputs
    pub console_log_inputs: Vec<Bytes>,
}

impl<HaltReasonT: HaltReasonTrait, LocalBlockT>
    DebugMineBlockResultAndState<HaltReasonT, LocalBlockT>
{
    /// Constructs a new instance from a [`MineBlockResultAndState`],
    /// transaction traces, and decoded console log messages.
    pub fn new(
        result: BuiltBlockAndState<HaltReasonT, LocalBlockT>,
        transaction_traces: Vec<SparsedTraceArena>,
        console_log_decoded_messages: Vec<Bytes>,
    ) -> Self {
        Self {
            block: result.block,
            state: result.state,
            state_diff: result.state_diff,
            transaction_results: result.transaction_results,
            transaction_traces,
            console_log_inputs: console_log_decoded_messages,
        }
    }
}

/// Helper type for a chain-specific [`DebugMineBlockResult`].
pub type DebugMineBlockResultForChainSpec<ChainSpecT> = DebugMineBlockResult<
    Arc<<ChainSpecT as BlockChainSpec>::Block>,
    <ChainSpecT as ChainSpec>::HaltReason,
    <ChainSpecT as ChainSpec>::SignedTransaction,
>;

/// The result of mining a block in debug mode, after having been committed to
/// the blockchain.
#[derive(Clone, Debug)]
pub struct DebugMineBlockResult<BlockT, HaltReasonT: HaltReasonTrait, SignedTransactionT> {
    /// Mined block
    pub block: BlockT,
    /// Transaction results
    pub transaction_results: Vec<ExecutionResult<HaltReasonT>>,
    /// Transaction traces
    pub transaction_traces: Vec<SparsedTraceArena>,
    /// Encoded `console.log` call inputs
    pub console_log_inputs: Vec<Bytes>,
    phantom: PhantomData<SignedTransactionT>,
}

impl<BlockT, HaltReasonT: HaltReasonTrait, SignedTransactionT>
    DebugMineBlockResult<BlockT, HaltReasonT, SignedTransactionT>
{
    /// Constructs a new instance.
    pub fn new(
        block: BlockT,
        transaction_results: Vec<ExecutionResult<HaltReasonT>>,
        transaction_traces: Vec<SparsedTraceArena>,
        console_log_inputs: Vec<Bytes>,
    ) -> Self {
        Self {
            block,
            transaction_results,
            transaction_traces,
            console_log_inputs,
            phantom: PhantomData,
        }
    }
}

impl<
        BlockT: Block<SignedTransactionT>,
        HaltReasonT: HaltReasonTrait,
        SignedTransactionT: ExecutableTransaction,
    > DebugMineBlockResult<BlockT, HaltReasonT, SignedTransactionT>
{
    /// Whether the block contains a transaction with the given hash.
    pub fn has_transaction(&self, transaction_hash: &B256) -> bool {
        self.block
            .transactions()
            .iter()
            .any(|tx| *tx.transaction_hash() == *transaction_hash)
    }
}
