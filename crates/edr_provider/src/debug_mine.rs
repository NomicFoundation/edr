use core::fmt::Debug;
use std::sync::Arc;

use derive_where::derive_where;
use edr_eth::{result::ExecutionResult, transaction::ExecutableTransaction, Bytes, B256};
use edr_evm::{
    spec::RuntimeSpec,
    state::{StateDiff, SyncState},
    trace::Trace,
    EthLocalBlock, MineBlockResultAndState, SyncBlock,
};

/// The result of mining a block, including the state, in debug mode. This
/// result needs to be inserted into the blockchain to be persistent.
pub struct DebugMineBlockResultAndState<ChainSpecT: RuntimeSpec, StateErrorT> {
    /// Mined block
    pub block: EthLocalBlock<ChainSpecT>,
    /// State after mining the block
    pub state: Box<dyn SyncState<StateErrorT>>,
    /// State diff applied by block
    pub state_diff: StateDiff,
    /// Transaction results
    pub transaction_results: Vec<ExecutionResult<ChainSpecT::HaltReason>>,
    /// Transaction traces
    pub transaction_traces: Vec<Trace<ChainSpecT::HaltReason>>,
    /// Encoded `console.log` call inputs
    pub console_log_inputs: Vec<Bytes>,
}

impl<ChainSpecT: RuntimeSpec, StateErrorT> DebugMineBlockResultAndState<ChainSpecT, StateErrorT> {
    /// Constructs a new instance from a [`MineBlockResultAndState`],
    /// transaction traces, and decoded console log messages.
    pub fn new(
        result: MineBlockResultAndState<ChainSpecT, StateErrorT>,
        transaction_traces: Vec<Trace<ChainSpecT::HaltReason>>,
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
pub struct DebugMineBlockResult<ChainSpecT: RuntimeSpec, BlockchainErrorT> {
    /// Mined block
    pub block: Arc<dyn SyncBlock<ChainSpecT, Error = BlockchainErrorT>>,
    /// Transaction results
    pub transaction_results: Vec<ExecutionResult<ChainSpecT::HaltReason>>,
    /// Transaction traces
    pub transaction_traces: Vec<Trace<ChainSpecT::HaltReason>>,
    /// Encoded `console.log` call inputs
    pub console_log_inputs: Vec<Bytes>,
}

impl<ChainSpecT: RuntimeSpec, BlockchainErrorT> DebugMineBlockResult<ChainSpecT, BlockchainErrorT> {
    /// Whether the block contains a transaction with the given hash.
    pub fn has_transaction(&self, transaction_hash: &B256) -> bool {
        self.block
            .transactions()
            .iter()
            .any(|tx| *tx.transaction_hash() == *transaction_hash)
    }
}
