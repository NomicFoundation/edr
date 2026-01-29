use core::fmt::Debug;
use std::{marker::PhantomData, sync::Arc};

use edr_block_api::Block;
use edr_block_builder_api::BuiltBlockAndState;
use edr_chain_spec::{ChainSpec, ExecutableTransaction, HaltReasonTrait};
use edr_chain_spec_block::BlockChainSpec;
use edr_chain_spec_evm::result::ExecutionResult;
use edr_primitives::{Address, Bytes, HashMap, HashSet, B256};
use edr_state_api::{DynState, StateDiff};
use foundry_evm_traces::CallTraceArena;

/// Helper type for a chain-specific [`DebugMineBlockResult`].
pub type DebugMineBlockResultForChainSpec<ChainSpecT> = DebugMineBlockResult<
    Arc<<ChainSpecT as BlockChainSpec>::Block>,
    <ChainSpecT as ChainSpec>::HaltReason,
    <ChainSpecT as ChainSpec>::SignedTransaction,
>;

/// The result of mining a block in debug mode, after having been committed to
/// the blockchain.
#[derive(Clone, Debug)]
pub struct DebugMineBlockResult<
    BlockT: Block<SignedTransactionT>,
    HaltReasonT: HaltReasonTrait,
    SignedTransactionT,
> {
    /// Mined block
    pub block: BlockT,
    /// Encoded `console.log` call inputs
    pub console_log_inputs: Vec<Bytes>,
    /// The set of precompile addresses that were available during execution.
    pub precompile_addresses: HashSet<Address>,
    /// Transaction results
    pub transaction_results: Vec<ExecutionResult<HaltReasonT>>,
    /// Transaction call trace arenas
    pub transaction_call_trace_arenas: Vec<CallTraceArena>,
    /// Mapping of contract address to executed bytecode per transaction
    pub transaction_address_to_executed_code: Vec<HashMap<Address, Bytes>>,
    phantom: PhantomData<SignedTransactionT>,
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
