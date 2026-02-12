use std::{borrow::Cow, collections::HashSet};

use edr_solidity::solidity_stack_trace::{StackTraceCreationError, StackTraceEntry};
use foundry_evm_core::{
    backend::IndeterminismReasons,
    evm_context::{
        BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr,
        TransactionErrorTrait,
    },
};
use revm::context::result::HaltReasonTr;

use crate::executors::{EvmError, ExecutorBuilderError};

/// Stack trace generation error during re-execution.
#[derive(Clone, Debug, thiserror::Error)]
pub enum SolidityTestStackTraceError<HaltReasonT> {
    #[error(transparent)]
    Creation(#[from] StackTraceCreationError<HaltReasonT>),
    #[error("Unexpected EVM execution error: {0}")]
    Evm(String),
    #[error("Test setup unexpectedly failed during execution with revert reason: {0}")]
    FailingSetup(String),
    #[error(transparent)]
    ExecutorBuilder(#[from] ExecutorBuilderError),
}

impl<HaltReasonT> SolidityTestStackTraceError<HaltReasonT> {
    pub fn map_halt_reason<
        ConversionFnT: Copy + Fn(HaltReasonT) -> NewHaltReasonT,
        NewHaltReasonT,
    >(
        self,
        conversion_fn: ConversionFnT,
    ) -> SolidityTestStackTraceError<NewHaltReasonT> {
        match self {
            SolidityTestStackTraceError::Creation(error) => {
                SolidityTestStackTraceError::Creation(error.map_halt_reason(conversion_fn))
            }
            SolidityTestStackTraceError::Evm(err) => SolidityTestStackTraceError::Evm(err),
            SolidityTestStackTraceError::FailingSetup(reason) => {
                SolidityTestStackTraceError::FailingSetup(reason)
            }
            SolidityTestStackTraceError::ExecutorBuilder(err) => {
                SolidityTestStackTraceError::ExecutorBuilder(err)
            }
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
    > for SolidityTestStackTraceError<HaltReasonT>
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

/// The possible outcomes from trying to compute a stack trace for Solidity
/// tests.
#[derive(Clone, Debug)]
pub enum SolidityTestStackTraceResult<HaltReasonT> {
    /// The stack trace result
    Success(Vec<StackTraceEntry>),
    /// We couldn't generate stack traces, because an unexpected error occurred.
    Error(SolidityTestStackTraceError<HaltReasonT>),
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

impl<HaltReasonT> SolidityTestStackTraceResult<HaltReasonT> {
    pub fn map_halt_reason<
        ConversionFnT: Copy + Fn(HaltReasonT) -> NewHaltReasonT,
        NewHaltReasonT,
    >(
        self,
        conversion_fn: ConversionFnT,
    ) -> SolidityTestStackTraceResult<NewHaltReasonT> {
        match self {
            SolidityTestStackTraceResult::Success(stack_trace) => {
                SolidityTestStackTraceResult::Success(stack_trace)
            }
            SolidityTestStackTraceResult::Error(error) => {
                SolidityTestStackTraceResult::Error(error.map_halt_reason(conversion_fn))
            }
            SolidityTestStackTraceResult::HeuristicFailed => {
                SolidityTestStackTraceResult::HeuristicFailed
            }
            SolidityTestStackTraceResult::UnsafeToReplay {
                global_fork_latest,
                impure_cheatcodes,
            } => SolidityTestStackTraceResult::UnsafeToReplay {
                global_fork_latest,
                impure_cheatcodes,
            },
        }
    }
}

impl<HaltReasonT: HaltReasonTr>
    From<Result<Vec<StackTraceEntry>, SolidityTestStackTraceError<HaltReasonT>>>
    for SolidityTestStackTraceResult<HaltReasonT>
{
    fn from(value: Result<Vec<StackTraceEntry>, SolidityTestStackTraceError<HaltReasonT>>) -> Self {
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

impl<HaltReasonT: HaltReasonTr> From<IndeterminismReasons>
    for SolidityTestStackTraceResult<HaltReasonT>
{
    fn from(value: IndeterminismReasons) -> Self {
        Self::UnsafeToReplay {
            global_fork_latest: value.global_fork_latest,
            impure_cheatcodes: value.impure_cheatcodes,
        }
    }
}
