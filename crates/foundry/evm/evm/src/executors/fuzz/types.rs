use alloy_primitives::Bytes;
use foundry_evm_core::backend::IndeterminismReasons;
use foundry_evm_coverage::HitMaps;
use foundry_evm_fuzz::FuzzCase;
use foundry_evm_traces::CallTraceArena;
use revm::interpreter::InstructionResult;

use crate::executors::RawCallResult;

/// Returned by a single fuzz in the case of a successful run
#[derive(Debug)]
pub struct CaseOutcome {
    /// Data of a single fuzz test case
    pub case: FuzzCase,
    /// The traces of the call
    pub traces: Option<CallTraceArena>,
    /// The coverage info collected during the call
    pub coverage: Option<HitMaps>,
}

/// Returned by a single fuzz when a counterexample has been discovered
#[derive(Debug)]
pub struct CounterExampleOutcome {
    /// Minimal reproduction test case for failing test
    pub counterexample: CounterExampleData,
    /// The status of the call
    pub exit_reason: InstructionResult,
}

#[derive(Debug)]
pub struct CounterExampleData {
    /// The calldata of the call
    pub calldata: Bytes,
    /// The call result
    pub call: RawCallResult,
    /// If re-executing the counter example is not guaranteed to yield the same
    /// results, this field contains the reason why.
    pub indeterminism_reasons: Option<IndeterminismReasons>,
}

/// Outcome of a single fuzz
#[derive(Debug)]
pub enum FuzzOutcome {
    Case(CaseOutcome),
    CounterExample(CounterExampleOutcome),
}
