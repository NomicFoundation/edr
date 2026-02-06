use std::fmt::Debug;

use alloy_rpc_types_trace::geth::{GethDebugTracingOptions, GethTrace};
use edr_block_builder_api::{DatabaseComponents, WrapDatabaseRef};
use edr_block_header::BlockHeader;
use edr_blockchain_api::{r#dyn::DynBlockchainError, BlockHashByNumber};
use edr_chain_spec::{ChainSpec, EvmSpecId, ExecutableTransaction as _, TransactionValidation};
use edr_chain_spec_block::BlockChainSpec;
use edr_chain_spec_evm::{BlockEnvTrait as _, CfgEnv, DatabaseComponentError, TransactionError};
use edr_evm::{dry_run_with_inspector, run};
use edr_primitives::{HashMap, B256, U256};
use edr_runtime::inspector::DualInspector;
use edr_state_api::{DynState, StateError};
use foundry_evm_traces::CallTraceArena;
use revm_inspectors::tracing::{DebugInspector, DebugInspectorError, MuxError, TransactionContext};

use crate::{
    error::{JsonRpcError, INTERNAL_ERROR, INVALID_PARAMS},
    observability::{EvmObservedData, EvmObserver, EvmObserverCollectionError, EvmObserverConfig},
};

/// Get trace output for `debug_traceTransaction`
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
#[allow(clippy::too_many_arguments)]
pub fn debug_trace_transaction<'header, ChainSpecT: BlockChainSpec<SignedTransaction: Clone>>(
    blockchain: &dyn BlockHashByNumber<Error = DynBlockchainError>,
    // Take ownership of the state so that we can apply throw-away modifications on it
    mut state: Box<dyn DynState>,
    evm_config: CfgEnv<ChainSpecT::Hardfork>,
    tracing_options: GethDebugTracingOptions,
    block: ChainSpecT::BlockEnv<'header, BlockHeader>,
    transactions: Vec<ChainSpecT::SignedTransaction>,
    transaction_hash: &B256,
    observer_config: EvmObserverConfig,
) -> Result<DebugTraceResultWithCallTraces, DebugTraceErrorForChainSpec<ChainSpecT>> {
    let evm_spec_id = evm_config.spec.into();
    if evm_spec_id < EvmSpecId::SPURIOUS_DRAGON {
        // Matching Hardhat Network behaviour: https://github.com/NomicFoundation/hardhat/blob/af7e4ce6a18601ec9cd6d4aa335fa7e24450e638/packages/hardhat-core/src/internal/hardhat-network/provider/vm/ethereumjs.ts#L427
        return Err(DebugTraceError::InvalidSpecId {
            spec_id: evm_spec_id,
        });
    }

    let block_number = block.number();
    let block_hash = blockchain
        .block_hash_by_number(block_number.try_into().expect("block number too large"))
        .map_err(DebugTraceError::Blockchain)?;

    for (transaction_index, transaction) in transactions.into_iter().enumerate() {
        if transaction.transaction_hash() == transaction_hash {
            let mut debug_inspector = DebugInspector::new(tracing_options)
                .map_err(DebugTraceError::from_debug_inspector_creation_error)?;

            let include_call_traces = observer_config.include_call_traces;
            let mut evm_observer = EvmObserver::new(observer_config);

            let transaction_hash = *transaction.transaction_hash();
            let result = dry_run_with_inspector::<ChainSpecT, _, _, _, _>(
                blockchain,
                state.as_ref(),
                evm_config,
                transaction.clone(),
                &block,
                &HashMap::default(),
                &mut DualInspector::new(&mut debug_inspector, &mut evm_observer),
            )?;

            let EvmObservedData {
                address_to_executed_code: _,
                call_trace_arena,
                encoded_console_logs: _,
            } = evm_observer.collect_and_report(&result.precompile_addresses)?;

            let mut database = WrapDatabaseRef(DatabaseComponents {
                blockchain,
                state: state.as_ref(),
            });

            let call_trace_arenas =
                if include_call_traces.should_include(|| !result.result.is_success()) {
                    vec![call_trace_arena]
                } else {
                    Vec::new()
                };

            let geth_trace = debug_inspector
                .get_result(
                    Some(TransactionContext {
                        block_hash: Some(block_hash),
                        tx_index: Some(transaction_index),
                        tx_hash: Some(transaction_hash),
                    }),
                    &transaction,
                    &block,
                    &result.into_result_and_state(),
                    &mut database,
                )
                .map_err(DebugTraceError::from_debug_inspector_result_error)?;

            return Ok(DebugTraceResultWithCallTraces {
                call_trace_arenas,
                result: geth_trace,
            });
        } else {
            run::<ChainSpecT, _, _, _>(
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
        block_number,
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
    #[error(transparent)]
    MuxInspector(#[from] MuxError),
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
    /// Converts from a `DebugInspectorError` that occurs when calling
    /// `DebugInspector::new`.
    pub fn from_debug_inspector_creation_error(error: DebugInspectorError) -> Self {
        match error {
            DebugInspectorError::InvalidTracerConfig => DebugTraceError::InvalidTracerConfig,
            DebugInspectorError::JsTracerNotEnabled => DebugTraceError::JsTracerNotEnabled,
            DebugInspectorError::MuxInspector(error) => DebugTraceError::MuxInspector(error),
            DebugInspectorError::UnsupportedTracer => DebugTraceError::UnsupportedTracer,
            DebugInspectorError::Database(_) => {
                unreachable!("Database errors should not occur while calling `DebugInspector::new`")
            }
        }
    }

    /// Converts from a `DebugInspectorError` that occurs when calling
    /// `DebugInspector::get_result`.
    pub fn from_debug_inspector_result_error(
        error: DebugInspectorError<DatabaseComponentError<DynBlockchainError, StateError>>,
    ) -> Self {
        match error {
            DebugInspectorError::Database(DatabaseComponentError::Blockchain(error)) => {
                DebugTraceError::Blockchain(error)
            }
            DebugInspectorError::Database(DatabaseComponentError::State(error)) => {
                DebugTraceError::State(error)
            }
            DebugInspectorError::InvalidTracerConfig
            | DebugInspectorError::JsTracerNotEnabled
            | DebugInspectorError::MuxInspector(_)
            | DebugInspectorError::UnsupportedTracer => {
                unreachable!(
                    "These `DebugInspectorError`s should not occur while calling `DebugInspector::::get_result`"
                )
            }
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
    pub call_trace_arenas: Vec<CallTraceArena>,
    /// The result of the transaction.
    pub result: GethTrace,
}
