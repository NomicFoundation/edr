use alloy_primitives::{Bytes, Log};
use derive_where::derive_where;
use foundry_evm_core::{
    backend::IndeterminismReasons,
    evm_context::{
        BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr,
        TransactionErrorTrait,
    },
};
use foundry_evm_coverage::HitMaps;
use foundry_evm_fuzz::FuzzCase;
use foundry_evm_traces::SparsedTraceArena;
use revm::{context::result::HaltReasonTr, interpreter::InstructionResult};

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
#[derive_where(Debug; BlockT, HardforkT, TxT)]
pub struct CounterExampleOutcome<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
> {
    /// Minimal reproduction test case for failing test.
    pub counterexample: CounterExampleData<
        BlockT,
        TxT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
    >,
    /// The status of the call.
    pub exit_reason: InstructionResult,
}

#[derive_where(Debug, Default; BlockT, HardforkT, TxT)]
pub struct CounterExampleData<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
> {
    /// The calldata of the call
    pub calldata: Bytes,
    /// The call result
    pub call: RawCallResult<
        BlockT,
        TxT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
    >,
    /// If re-executing the counter example is not guaranteed to yield the same
    /// results, this field contains the reason why.
    pub indeterminism_reasons: Option<IndeterminismReasons>,
}

/// Outcome of a single fuzz
#[derive_where(Debug; BlockT, HardforkT, TxT)]
#[allow(clippy::large_enum_variant)]
pub enum FuzzOutcome<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
> {
    Case(CaseOutcome),
    CounterExample(
        CounterExampleOutcome<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ),
}
