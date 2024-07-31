use core::fmt::Debug;
use std::sync::Arc;

use derive_where::derive_where;
use edr_eth::{result::ExecutionResult, transaction::SignedTransaction, Bytes, B256};
use edr_evm::{
    chain_spec::ChainSpec,
    state::{StateDiff, SyncState},
    trace::Trace,
    LocalBlock, MineBlockResultAndState, SyncBlock,
};

/// The result of mining a block, including the state, in debug mode. This
/// result needs to be inserted into the blockchain to be persistent.
pub struct DebugMineBlockResultAndState<ChainSpecT: ChainSpec, StateErrorT> {
    /// Mined block
    pub block: LocalBlock<ChainSpecT>,
    /// State after mining the block
    pub state: Box<dyn SyncState<StateErrorT>>,
    /// State diff applied by block
    pub state_diff: StateDiff,
    /// Transaction results
    pub transaction_results: Vec<ExecutionResult<ChainSpecT>>,
    /// Transaction traces
    pub transaction_traces: Vec<Trace<ChainSpecT>>,
    /// Encoded `console.log` call inputs
    pub console_log_inputs: Vec<Bytes>,
}

impl<ChainSpecT: ChainSpec, StateErrorT> DebugMineBlockResultAndState<ChainSpecT, StateErrorT> {
    /// Constructs a new instance from a [`MineBlockResultAndState`],
    /// transaction traces, and decoded console log messages.
    pub fn new(
        result: MineBlockResultAndState<ChainSpecT, StateErrorT>,
        transaction_traces: Vec<Trace<ChainSpecT>>,
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

/// The result of mining a block in debug mode, after having been committed to
/// the blockchain.
#[derive(Debug)]
#[derive_where(Clone; ChainSpecT::HaltReason)]
pub struct DebugMineBlockResult<ChainSpecT: ChainSpec, BlockchainErrorT> {
    /// Mined block
    pub block: Arc<dyn SyncBlock<ChainSpecT, Error = BlockchainErrorT>>,
    /// Transaction results
    pub transaction_results: Vec<ExecutionResult<ChainSpecT>>,
    /// Transaction traces
    pub transaction_traces: Vec<Trace<ChainSpecT>>,
    /// Encoded `console.log` call inputs
    pub console_log_inputs: Vec<Bytes>,
}

impl<ChainSpecT: ChainSpec, BlockchainErrorT> DebugMineBlockResult<ChainSpecT, BlockchainErrorT> {
    /// Whether the block contains a transaction with the given hash.
    pub fn has_transaction(&self, transaction_hash: &B256) -> bool {
        self.block
            .transactions()
            .iter()
            .any(|tx| *tx.transaction_hash() == *transaction_hash)
    }
}
