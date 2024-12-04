use core::fmt::Debug;
use std::sync::Arc;

use edr_eth::{
    result::ExecutionResult,
    spec::{ChainSpec, HaltReasonTrait},
    transaction::ExecutableTransaction as _,
    Bytes, B256,
};
use edr_evm::{
    spec::RuntimeSpec,
    state::{StateDiff, SyncState},
    trace::Trace,
    Block as _, MineBlockResultAndState,
};

/// The result of mining a block, including the state, in debug mode. This
/// result needs to be inserted into the blockchain to be persistent.
pub struct DebugMineBlockResultAndState<HaltReasonT: HaltReasonTrait, LocalBlockT, StateErrorT> {
    /// Mined block
    pub block: LocalBlockT,
    /// State after mining the block
    pub state: Box<dyn SyncState<StateErrorT>>,
    /// State diff applied by block
    pub state_diff: StateDiff,
    /// Transaction results
    pub transaction_results: Vec<ExecutionResult<HaltReasonT>>,
    /// Transaction traces
    pub transaction_traces: Vec<Trace<HaltReasonT>>,
    /// Encoded `console.log` call inputs
    pub console_log_inputs: Vec<Bytes>,
}

impl<HaltReasonT: HaltReasonTrait, LocalBlockT, StateErrorT>
    DebugMineBlockResultAndState<HaltReasonT, LocalBlockT, StateErrorT>
{
    /// Constructs a new instance from a [`MineBlockResultAndState`],
    /// transaction traces, and decoded console log messages.
    pub fn new(
        result: MineBlockResultAndState<HaltReasonT, LocalBlockT, StateErrorT>,
        transaction_traces: Vec<Trace<HaltReasonT>>,
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

/// Helper trait for a chain-specific [`DebugMineBlockResult`].
pub type DebugMineBlockResultForChainSpec<ChainSpecT> =
    DebugMineBlockResult<<ChainSpecT as RuntimeSpec>::Block, <ChainSpecT as ChainSpec>::HaltReason>;

/// The result of mining a block in debug mode, after having been committed to
/// the blockchain.
#[derive(Clone, Debug)]
pub struct DebugMineBlockResult<BlockT, HaltReasonT: HaltReasonTrait> {
    /// Mined block
    pub block: Arc<BlockT>,
    /// Transaction results
    pub transaction_results: Vec<ExecutionResult<HaltReasonT>>,
    /// Transaction traces
    pub transaction_traces: Vec<Trace<HaltReasonT>>,
    /// Encoded `console.log` call inputs
    pub console_log_inputs: Vec<Bytes>,
}

impl<BlockT, HaltReasonT: HaltReasonTrait> DebugMineBlockResult<BlockT, HaltReasonT> {
    /// Whether the block contains a transaction with the given hash.
    pub fn has_transaction(&self, transaction_hash: &B256) -> bool {
        self.block
            .transactions()
            .iter()
            .any(|tx| *tx.transaction_hash() == *transaction_hash)
    }
}
