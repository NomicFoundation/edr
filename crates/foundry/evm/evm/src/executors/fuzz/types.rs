use alloy_primitives::{Bytes, Log};
use foundry_evm_core::{
    backend::IndeterminismReasons,
    evm_context::{BlockEnvTr, HardforkTr, TransactionEnvTr},
};
use foundry_evm_coverage::HitMaps;
use foundry_evm_fuzz::FuzzCase;
use foundry_evm_traces::SparsedTraceArena;
use revm::interpreter::InstructionResult;

use crate::executors::RawCallResult;

/// Returned by a single fuzz in the case of a successful run
#[derive(Debug)]
pub struct CaseOutcome {
    /// Data of a single fuzz test case.
    pub case: FuzzCase,
    /// The traces of the call.
    pub traces: Option<SparsedTraceArena>,
    /// The coverage info collected during the call.
    pub coverage: Option<HitMaps>,
    /// logs of a single fuzz test case.
    pub logs: Vec<Log>,
}

/// Returned by a single fuzz when a counterexample has been discovered
#[derive(Debug)]
pub struct CounterExampleOutcome<BlockT: BlockEnvTr, TxT: TransactionEnvTr, HardforkT: HardforkTr> {
    /// Minimal reproduction test case for failing test.
    pub counterexample: CounterExampleData<BlockT, TxT, HardforkT>,
    /// The status of the call.
    pub exit_reason: InstructionResult,
}

#[derive(Debug, Default)]
pub struct CounterExampleData<BlockT: BlockEnvTr, TxT: TransactionEnvTr, HardforkT: HardforkTr> {
    /// The calldata of the call
    pub calldata: Bytes,
    /// The call result
    pub call: RawCallResult<BlockT, TxT, HardforkT>,
    /// If re-executing the counter example is not guaranteed to yield the same
    /// results, this field contains the reason why.
    pub indeterminism_reasons: Option<IndeterminismReasons>,
}

/// Outcome of a single fuzz
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum FuzzOutcome<BlockT: BlockEnvTr, TxT: TransactionEnvTr, HardforkT: HardforkTr> {
    Case(CaseOutcome),
    CounterExample(CounterExampleOutcome<BlockT, TxT, HardforkT>),
}
