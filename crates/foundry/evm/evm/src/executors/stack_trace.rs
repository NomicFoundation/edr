use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
};

use alloy_primitives::{Address, Bytes};
use edr_solidity::{
    contract_decoder::{ContractDecoderError, NestedTraceDecoder},
    nested_trace::NestedTrace,
    solidity_stack_trace::StackTraceEntry,
    solidity_tracer::{self, SolidityTracerError},
};
use foundry_evm_core::{
    backend::IndeterminismReasons,
    evm_context::{
        BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr,
        TransactionErrorTrait,
    },
};
use foundry_evm_traces::{SparsedTraceArena, TraceKind};
use revm::context::result::HaltReasonTr;

use crate::executors::{EvmError, ExecutorBuilderError};

/// Stack trace generation error during re-execution.
#[derive(Clone, Debug, thiserror::Error)]
pub enum StackTraceError<HaltReasonT> {
    #[error(transparent)]
    ContractDecoder(#[from] ContractDecoderError),
    #[error("Unexpected EVM execution error: {0}")]
    Evm(String),
    #[error("Test setup unexpectedly failed during execution with revert reason: {0}")]
    FailingSetup(String),
    #[error(transparent)]
    Tracer(#[from] SolidityTracerError<HaltReasonT>),
    #[error(transparent)]
    ExecutorBuilder(#[from] ExecutorBuilderError),
}

impl<HaltReasonT> StackTraceError<HaltReasonT> {
    pub fn map_halt_reason<
        ConversionFnT: Copy + Fn(HaltReasonT) -> NewHaltReasonT,
        NewHaltReasonT,
    >(
        self,
        conversion_fn: ConversionFnT,
    ) -> StackTraceError<NewHaltReasonT> {
        match self {
            StackTraceError::ContractDecoder(err) => StackTraceError::ContractDecoder(err),
            StackTraceError::Evm(err) => StackTraceError::Evm(err),
            StackTraceError::FailingSetup(reason) => StackTraceError::FailingSetup(reason),
            StackTraceError::Tracer(err) => {
                StackTraceError::Tracer(err.map_halt_reason(conversion_fn))
            }
            StackTraceError::ExecutorBuilder(err) => StackTraceError::ExecutorBuilder(err),
        }
    }
}

// `EvmError` is not `Clone`
impl<
        BlockT: BlockEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        TxT: TransactionEnvTr,
    >
    From<
        EvmError<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    > for StackTraceError<HaltReasonT>
{
    fn from(
        value: EvmError<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Self {
        Self::Evm(value.to_string())
    }
}

/// Compute stack trace based on execution traces.
/// Assumes last trace is the error one. This is important for invariant tests
/// where there might be multiple errors traces. Returns `None` if `traces` is
/// empty.
pub fn get_stack_trace<
    HaltReasonT: HaltReasonTr,
    NestedTraceDecoderT: NestedTraceDecoder<HaltReasonT>,
>(
    contract_decoder: &NestedTraceDecoderT,
    traces: &[(TraceKind, SparsedTraceArena)],
) -> Result<Option<Vec<StackTraceEntry>>, StackTraceError<HaltReasonT>> {
    let mut address_to_creation_code = HashMap::new();
    let mut address_to_runtime_code = HashMap::new();

    for (_, trace) in traces {
        for node in trace.nodes() {
            let address = node.trace.address;
            if node.trace.kind.is_any_create() {
                address_to_creation_code.insert(address, &node.trace.data);
                address_to_runtime_code.insert(address, &node.trace.output);
            }
        }
    }

    if let Some((_, last_trace)) = traces.last() {
        let trace = NestedTrace::from_call_trace_arena(
            &address_to_creation_code,
            &address_to_runtime_code,
            last_trace,
        )
        .map_err(|err| StackTraceError::Evm(err.to_string()))?;
        let trace = contract_decoder.try_to_decode_nested_trace(trace)?;
        let stack_trace = solidity_tracer::get_stack_trace(trace)?;
        Ok(Some(stack_trace))
    } else {
        Ok(None)
    }
}


/// The possible outcomes from computing stack traces.
#[derive(Clone, Debug)]
pub enum StackTraceResult<HaltReasonT> {
    /// The stack trace result
    Success(Vec<StackTraceEntry>),
    /// We couldn't generate stack traces, because an unexpected error occurred.
    Error(StackTraceError<HaltReasonT>),
    HeuristicFailed,
    /// We couldn't generate stack traces, because the test execution is unsafe
    /// to replay due to indeterminism. This can be caused by either
    /// specifying a fork url without a fork block number in the test runner
    /// config or using impure cheatcodes.
    UnsafeToReplay {
        /// Indeterminism due to specifying a fork url without a fork block
        /// number in the test runner config
        global_fork_latest: bool,
        /// The list of executed impure cheatcode signatures. We collect
        /// function signatures instead of function names as whether a cheatcode
        /// is impure can depend on the arguments it takes (e.g. `createFork`
        /// without a second argument means implicitly fork from “latest”).
        /// Example signature: `function createSelectFork(string calldata
        /// urlOrAlias) external returns (uint256 forkId);`.
        impure_cheatcodes: HashSet<Cow<'static, str>>,
    },
}

impl<HaltReasonT> StackTraceResult<HaltReasonT> {
    pub fn map_halt_reason<
        ConversionFnT: Copy + Fn(HaltReasonT) -> NewHaltReasonT,
        NewHaltReasonT,
    >(
        self,
        conversion_fn: ConversionFnT,
    ) -> StackTraceResult<NewHaltReasonT> {
        match self {
            StackTraceResult::Success(stack_trace) => StackTraceResult::Success(stack_trace),
            StackTraceResult::Error(error) => {
                StackTraceResult::Error(error.map_halt_reason(conversion_fn))
            }
            StackTraceResult::HeuristicFailed => StackTraceResult::HeuristicFailed,
            StackTraceResult::UnsafeToReplay {
                global_fork_latest,
                impure_cheatcodes,
            } => StackTraceResult::UnsafeToReplay {
                global_fork_latest,
                impure_cheatcodes,
            },
        }
    }
}

impl<HaltReasonT: HaltReasonTr> From<Result<Vec<StackTraceEntry>, StackTraceError<HaltReasonT>>>
    for StackTraceResult<HaltReasonT>
{
    fn from(value: Result<Vec<StackTraceEntry>, StackTraceError<HaltReasonT>>) -> Self {
        match value {
            Ok(stack_trace) => {
                if stack_trace.is_empty() {
                    Self::HeuristicFailed
                } else {
                    Self::Success(stack_trace)
                }
            }
            Err(error) => Self::Error(error),
        }
    }
}

impl<HaltReasonT: HaltReasonTr> From<IndeterminismReasons> for StackTraceResult<HaltReasonT> {
    fn from(value: IndeterminismReasons) -> Self {
        Self::UnsafeToReplay {
            global_fork_latest: value.global_fork_latest,
            impure_cheatcodes: value.impure_cheatcodes,
        }
    }
}
