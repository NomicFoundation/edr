use std::fmt::Debug;

use alloy_rpc_types_trace::geth::{GethDebugTracingOptions, GethTrace};
use edr_block_header::BlockHeader;
use edr_blockchain_api::{r#dyn::DynBlockchainError, BlockHashByNumber};
use edr_chain_spec::{ChainSpec, EvmSpecId, ExecutableTransaction as _, TransactionValidation};
use edr_chain_spec_evm::{CfgEnv, DatabaseComponentError, TransactionError};
use edr_chain_spec_provider::ProviderChainSpec;
use edr_inspection::{
    BlockEnvOverrides, DebugTraceContext, DebugTraceRequest, DebugTracerError, InspectedExecution,
    InspectionError, InspectionRequest,
};
use edr_primitives::{HashMap, B256, U256};
use edr_solidity::recorded_trace_arena::RecordedTraceArena;
use edr_state_api::{DynState, StateError};

use crate::{
    error::{JsonRpcError, INTERNAL_ERROR, INVALID_PARAMS},
    observability::{EvmObserverCollectionError, EvmObserverConfig},
};

/// Get trace output for `debug_traceTransaction`
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
#[allow(clippy::too_many_arguments)]
pub fn debug_trace_transaction<'header, ChainSpecT: ProviderChainSpec<SignedTransaction: Clone>>(
    blockchain: &dyn BlockHashByNumber<Error = DynBlockchainError>,
    block_number: u64,
    // Take ownership of the state so that we can apply throw-away modifications on it
    mut state: Box<dyn DynState>,
    evm_config: CfgEnv<ChainSpecT::ProtocolHardfork>,
    tracing_options: GethDebugTracingOptions,
    block: ChainSpecT::BlockEnv<'header, BlockHeader>,
    transactions: Vec<ChainSpecT::SignedTransaction>,
    transaction_hash: &B256,
    observer_config: EvmObserverConfig,
) -> Result<DebugTraceResultWithCallTraces, DebugTraceErrorForChainSpec<ChainSpecT>> {
    let evm_spec_id: EvmSpecId = evm_config.spec.clone().into();
    if evm_spec_id < EvmSpecId::SPURIOUS_DRAGON {
        // Matching Hardhat Network behaviour: https://github.com/NomicFoundation/hardhat/blob/af7e4ce6a18601ec9cd6d4aa335fa7e24450e638/packages/hardhat-core/src/internal/hardhat-network/provider/vm/ethereumjs.ts#L427
        return Err(DebugTraceError::InvalidSpecId {
            spec_id: evm_spec_id,
        });
    }

    let block_hash = blockchain
        .block_hash_by_number(block_number)
        .map_err(DebugTraceError::Blockchain)?;

    for (transaction_index, transaction) in transactions.into_iter().enumerate() {
        if transaction.transaction_hash() == transaction_hash {
            let include_call_traces = observer_config.include_call_traces;
            let request = InspectionRequest {
                observer: observer_config,
                debug_trace: Some(DebugTraceRequest {
                    options: tracing_options,
                    context: Some(DebugTraceContext {
                        block_hash: Some(block_hash),
                        transaction_index: Some(transaction_index),
                        transaction_hash: Some(*transaction.transaction_hash()),
                    }),
                }),
            };

            let InspectedExecution {
                execution,
                observed,
                geth_trace,
            } = ChainSpecT::dry_run_with_inspection(
                blockchain,
                state.as_ref(),
                evm_config,
                transaction,
                &block,
                BlockEnvOverrides::default(),
                &HashMap::default(),
                request,
            )
            .map_err(DebugTraceError::from_inspection_error)?;

            let execution_trace =
                observed.into_call_traces(include_call_traces, execution.result.is_success());

            return Ok(DebugTraceResultWithCallTraces {
                execution_traces: execution_trace.into_iter().collect(),
                result: geth_trace.expect("a debug trace was requested"),
            });
        } else {
            ChainSpecT::run(
                blockchain,
                state.as_mut(),
                evm_config.clone(),
                transaction,
                &block,
                &HashMap::default(),
            )?;
        }
    }

    Err(DebugTraceError::InvalidTransactionHash {
        transaction_hash: *transaction_hash,
        block_number: U256::from(block_number),
    })
}

/// Helper type for a chain-specific [`DebugTraceError`].
pub type DebugTraceErrorForChainSpec<ChainSpecT> = DebugTraceError<
    <<ChainSpecT as ChainSpec>::SignedTransaction as TransactionValidation>::ValidationError,
>;

/// Debug trace error.
#[derive(Debug, thiserror::Error)]
pub enum DebugTraceError<TransactionValidationErrorT> {
    // TODO: This error should be caught when we originally parse the contract ABIs. Once we do
    // that, this variant should be removed from the enum.
    /// An error occurred while ABI decoding the traces due to invalid input.
    #[error(transparent)]
    AbiDecoding(serde_json::Error),
    /// Blockchain error.
    #[error(transparent)]
    Blockchain(DynBlockchainError),
    /// Invalid hardfork spec argument.
    #[error(
        "Invalid spec id: {spec_id:?}. `debug_traceTransaction` is not supported prior to Spurious Dragon"
    )]
    InvalidSpecId {
        /// The hardfork.
        spec_id: EvmSpecId,
    },
    /// Invalid tracer configuration
    #[error("invalid tracer config")]
    InvalidTracerConfig,
    /// Invalid transaction hash argument.
    #[error("Transaction hash {transaction_hash} not found in block {block_number}")]
    InvalidTransactionHash {
        /// The transaction hash.
        transaction_hash: B256,
        /// The block number.
        block_number: U256,
    },
    /// JS tracer is not enabled
    #[error("JS Tracer is not enabled")]
    JsTracerNotEnabled,
    /// Error from `MuxInspector`
    #[error("{0}")]
    MuxInspector(String),
    /// An error occurred while invoking a `SyncOnCollectedCoverageCallback`.
    #[error(transparent)]
    OnCollectedCoverageCallback(Box<dyn std::error::Error + Send + Sync>),
    /// State error.
    #[error(transparent)]
    State(StateError),
    /// Transaction error.
    #[error(transparent)]
    TransactionError(
        #[from]
        TransactionError<
            DatabaseComponentError<DynBlockchainError, StateError>,
            TransactionValidationErrorT,
        >,
    ),
    /// Unsupported tracer
    #[error("unsupported tracer")]
    UnsupportedTracer,
}

impl<TransactionValidationErrorT> DebugTraceError<TransactionValidationErrorT> {
    /// Converts from the error of an inspected execution.
    pub fn from_inspection_error(
        error: InspectionError<
            DatabaseComponentError<DynBlockchainError, StateError>,
            TransactionValidationErrorT,
        >,
    ) -> Self {
        match error {
            InspectionError::Collection(error) => error.into(),
            InspectionError::Database(DatabaseComponentError::Blockchain(error)) => {
                DebugTraceError::Blockchain(error)
            }
            InspectionError::Database(DatabaseComponentError::State(error)) => {
                DebugTraceError::State(error)
            }
            InspectionError::DebugTracer(error) => match error {
                DebugTracerError::InvalidTracerConfig => DebugTraceError::InvalidTracerConfig,
                DebugTracerError::JsTracerNotEnabled => DebugTraceError::JsTracerNotEnabled,
                DebugTracerError::MuxInspector(error) => DebugTraceError::MuxInspector(error),
                DebugTracerError::UnsupportedTracer => DebugTraceError::UnsupportedTracer,
            },
            InspectionError::Transaction(error) => DebugTraceError::TransactionError(error),
        }
    }
}

impl<TransactionValidationErrorT> From<EvmObserverCollectionError>
    for DebugTraceError<TransactionValidationErrorT>
{
    fn from(value: EvmObserverCollectionError) -> Self {
        match value {
            EvmObserverCollectionError::AbiDecoding(error) => DebugTraceError::AbiDecoding(error),
            EvmObserverCollectionError::OnCollectedCoverageCallback(error) => {
                DebugTraceError::OnCollectedCoverageCallback(error)
            }
        }
    }
}

impl<TransactionValidationErrorT> JsonRpcError for DebugTraceError<TransactionValidationErrorT> {
    fn error_code(&self) -> i16 {
        match self {
            DebugTraceError::InvalidTracerConfig
            | DebugTraceError::InvalidTransactionHash { .. }
            | DebugTraceError::JsTracerNotEnabled
            | DebugTraceError::MuxInspector(_)
            | DebugTraceError::UnsupportedTracer => INVALID_PARAMS,
            DebugTraceError::AbiDecoding(_)
            | DebugTraceError::InvalidSpecId { .. }
            | DebugTraceError::OnCollectedCoverageCallback(_)
            | DebugTraceError::Blockchain(_)
            | DebugTraceError::State(_)
            | DebugTraceError::TransactionError(_) => INTERNAL_ERROR,
        }
    }
}

/// Result of a `debug_traceTransaction` call with call trace.
pub struct DebugTraceResultWithCallTraces {
    /// The raw traces of the debugged transaction.
    pub execution_traces: Vec<RecordedTraceArena>,
    /// The result of the transaction.
    pub result: GethTrace,
}
