//! Inspection of transaction execution, requested as data.
//!
//! Callers describe what to observe with an [`InspectionRequest`] and receive
//! an [`InspectedExecution`]. Each chain drives its engine's inspectors behind
//! [`InspectionChainSpec`], so no inspector type crosses the chain boundary.

mod call_override;
mod console_log;
mod observer;
pub mod revm;

use alloy_rpc_types_trace::geth::{GethDebugTracingOptions, GethTrace};
use edr_blockchain_api::BlockHashByNumber;
use edr_chain_spec::{BlockEnvChainSpec, BlockEnvForHardfork, TransactionValidation};
use edr_chain_spec_evm::{CfgEnv, DatabaseComponentError, EvmChainSpec, TransactionError};
use edr_evm::{ExecutionResultAndStateWithMetadata, ExecutionResultWithMetadata};
use edr_precompile::PrecompileFn;
use edr_primitives::{Address, HashMap, B256};
use edr_state_api::{State, StateCommit};

pub use self::{
    call_override::{CallOverrideResult, Mocker, SyncCallOverride},
    console_log::ConsoleLogCollector,
    observer::{EvmObservedData, EvmObserver, EvmObserverCollectionError, EvmObserverConfig},
    revm::BlockEnvWithOverrides,
};

/// Changes applied to a block environment before running a transaction.
#[derive(Clone, Debug, Default)]
pub struct BlockEnvOverrides {
    /// Replaces the block's base fee; e.g. `Some(0)` for `eth_call`, to
    /// match Geth.
    pub base_fee: Option<u64>,
}

/// What to observe while running a transaction.
pub struct InspectionRequest {
    /// The observer that collects traces, console logs, executed code and
    /// coverage, and applies call overrides.
    pub observer: EvmObserverConfig,
    /// A Geth debug tracer to run alongside the observer.
    pub debug_trace: Option<DebugTraceRequest>,
}

/// A Geth debug tracer request, as used by `debug_trace*`.
pub struct DebugTraceRequest {
    pub options: GethDebugTracingOptions,
    /// Where the transaction sits in its block, if it was mined.
    pub context: Option<DebugTraceContext>,
}

/// Position of a traced transaction in its block.
#[derive(Clone, Copy, Debug, Default)]
pub struct DebugTraceContext {
    pub block_hash: Option<B256>,
    pub transaction_index: Option<usize>,
    pub transaction_hash: Option<B256>,
}

/// The result of an inspected execution and what was observed.
pub struct InspectedExecution<HaltReasonT> {
    pub execution: ExecutionResultAndStateWithMetadata<HaltReasonT>,
    pub observed: EvmObservedData,
    /// Present if a debug tracer was requested.
    pub geth_trace: Option<GethTrace>,
}

/// Error from the debug tracer.
#[derive(Debug, thiserror::Error)]
pub enum DebugTracerError {
    #[error("invalid tracer config")]
    InvalidTracerConfig,
    #[error("JS Tracer is not enabled")]
    JsTracerNotEnabled,
    #[error("{0}")]
    MuxInspector(String),
    #[error("unsupported tracer")]
    UnsupportedTracer,
}

/// Error from an inspected execution.
#[derive(Debug, thiserror::Error)]
pub enum InspectionError<DatabaseErrorT, TransactionValidationErrorT> {
    #[error(transparent)]
    Collection(#[from] EvmObserverCollectionError),
    /// Database error while building the debug trace.
    #[error(transparent)]
    Database(DatabaseErrorT),
    #[error(transparent)]
    DebugTracer(#[from] DebugTracerError),
    #[error(transparent)]
    Transaction(TransactionError<DatabaseErrorT, TransactionValidationErrorT>),
}

/// Helper type for a chain-specific [`InspectionError`].
pub type InspectionErrorForChainSpec<ChainSpecT, BlockchainErrorT, StateErrorT> = InspectionError<
    DatabaseComponentError<BlockchainErrorT, StateErrorT>,
    <<ChainSpecT as edr_chain_spec::ChainSpec>::SignedTransaction as TransactionValidation>::ValidationError,
>;

/// Runs a transaction in a chain's EVM, observed as described by a request.
pub trait InspectionChainSpec: EvmChainSpec + BlockEnvChainSpec {
    /// Runs a transaction without committing the state, observing it as
    /// described by `request`.
    #[allow(clippy::too_many_arguments, clippy::type_complexity)]
    fn dry_run_with_inspection<
        'block,
        BlockHeaderT: 'block + BlockEnvForHardfork<Self::ProtocolHardfork>,
        BlockchainT: BlockHashByNumber<Error: 'static + std::error::Error + Send + Sync> + Clone,
        StateT: State<Error: 'static + std::error::Error + Send + Sync> + Clone,
    >(
        blockchain: BlockchainT,
        state: StateT,
        cfg: CfgEnv<Self::ProtocolHardfork>,
        transaction: Self::SignedTransaction,
        block: &Self::BlockEnv<'block, BlockHeaderT>,
        block_overrides: BlockEnvOverrides,
        custom_precompiles: &HashMap<Address, PrecompileFn>,
        request: InspectionRequest,
    ) -> Result<
        InspectedExecution<Self::HaltReason>,
        InspectionErrorForChainSpec<Self, BlockchainT::Error, StateT::Error>,
    >;

    /// Runs a transaction and commits the state, without inspection.
    #[allow(clippy::type_complexity)]
    fn run<
        'block,
        BlockHeaderT: 'block + BlockEnvForHardfork<Self::ProtocolHardfork>,
        BlockchainT: BlockHashByNumber<Error: 'static + std::error::Error + Send + Sync>,
        StateT: State<Error: 'static + std::error::Error + Send + Sync> + StateCommit,
    >(
        blockchain: BlockchainT,
        state: StateT,
        cfg: CfgEnv<Self::ProtocolHardfork>,
        transaction: Self::SignedTransaction,
        block: &Self::BlockEnv<'block, BlockHeaderT>,
        custom_precompiles: &HashMap<Address, PrecompileFn>,
    ) -> Result<
        ExecutionResultWithMetadata<Self::HaltReason>,
        TransactionError<
            DatabaseComponentError<BlockchainT::Error, StateT::Error>,
            <Self::SignedTransaction as TransactionValidation>::ValidationError,
        >,
    >;
}
