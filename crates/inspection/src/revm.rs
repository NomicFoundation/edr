//! [`InspectionChainSpec`](crate::InspectionChainSpec) implementation shared by
//! chains that run on revm.

use edr_blockchain_api::BlockHashByNumber;
use edr_chain_spec_evm::{result::ExecutionResultAndState, BlockEnvTrait, CfgEnv, EvmChainSpec};
use edr_database_components::{DatabaseComponents, WrapDatabaseRef};
use edr_evm::{dry_run_with_inspector, ExecutionResultAndStateWithMetadata};
use edr_precompile::PrecompileFn;
use edr_primitives::{Address, HashMap};
use edr_runtime::inspector::DualInspector;
use edr_state_api::State;
use revm_inspectors::tracing::{DebugInspector, DebugInspectorError, TransactionContext};

use crate::{
    DebugTraceRequest, DebugTracerError, EvmObserver, InspectedExecution, InspectionError,
    InspectionErrorForChainSpec, InspectionRequest,
};

/// Runs a transaction on revm without committing the state, observing it as
/// described by `request`.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn dry_run_with_inspection<
    ChainSpecT: EvmChainSpec<SignedTransaction: Clone>,
    BlockT: BlockEnvTrait,
    BlockchainT: BlockHashByNumber<Error: 'static + std::error::Error + Send + Sync> + Clone,
    StateT: State<Error: 'static + std::error::Error + Send + Sync> + Clone,
>(
    blockchain: BlockchainT,
    state: StateT,
    cfg: CfgEnv<ChainSpecT::ProtocolHardfork>,
    transaction: ChainSpecT::SignedTransaction,
    block: BlockT,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
    request: InspectionRequest,
) -> Result<
    InspectedExecution<ChainSpecT::HaltReason>,
    InspectionErrorForChainSpec<ChainSpecT, BlockchainT::Error, StateT::Error>,
> {
    let InspectionRequest {
        observer_config,
        debug_trace,
    } = request;

    let mut observer = EvmObserver::new(observer_config);

    let Some(DebugTraceRequest { options, context }) = debug_trace else {
        let execution = dry_run_with_inspector::<ChainSpecT, _, _, _, _>(
            blockchain,
            state,
            cfg,
            transaction,
            &block,
            custom_precompiles,
            &mut observer,
        )
        .map_err(InspectionError::Transaction)?;

        let observed = observer.collect_and_report(&execution.precompile_addresses)?;

        return Ok(InspectedExecution {
            execution,
            observed,
            geth_trace: None,
        });
    };

    let mut debug_inspector = DebugInspector::new(options).map_err(from_creation_error)?;

    let execution = dry_run_with_inspector::<ChainSpecT, _, _, _, _>(
        blockchain.clone(),
        state.clone(),
        cfg,
        transaction.clone(),
        &block,
        custom_precompiles,
        &mut DualInspector::new(&mut debug_inspector, &mut observer),
    )
    .map_err(InspectionError::Transaction)?;

    let observed = observer.collect_and_report(&execution.precompile_addresses)?;

    let ExecutionResultAndStateWithMetadata {
        precompile_addresses,
        result,
        state: state_diff,
    } = execution;
    let result_and_state = ExecutionResultAndState {
        result,
        state: state_diff,
    };

    let mut database = WrapDatabaseRef(DatabaseComponents { blockchain, state });
    let geth_trace = debug_inspector
        .get_result(
            context.map(|context| TransactionContext {
                block_hash: context.block_hash,
                tx_index: context.transaction_index,
                tx_hash: context.transaction_hash,
            }),
            &transaction,
            &block,
            &result_and_state,
            &mut database,
        )
        .map_err(|error| match error {
            DebugInspectorError::Database(error) => InspectionError::Database(error),
            DebugInspectorError::InvalidTracerConfig
            | DebugInspectorError::JsTracerNotEnabled
            | DebugInspectorError::MuxInspector(_)
            | DebugInspectorError::UnsupportedTracer => {
                unreachable!("only database errors occur while building the debug trace")
            }
        })?;

    Ok(InspectedExecution {
        execution: ExecutionResultAndStateWithMetadata::new(result_and_state, precompile_addresses),
        observed,
        geth_trace: Some(geth_trace),
    })
}

fn from_creation_error<DatabaseErrorT>(
    error: DebugInspectorError<DatabaseErrorT>,
) -> DebugTracerError {
    match error {
        DebugInspectorError::InvalidTracerConfig => DebugTracerError::InvalidTracerConfig,
        DebugInspectorError::JsTracerNotEnabled => DebugTracerError::JsTracerNotEnabled,
        DebugInspectorError::MuxInspector(error) => {
            DebugTracerError::MuxInspector(error.to_string())
        }
        DebugInspectorError::UnsupportedTracer => DebugTracerError::UnsupportedTracer,
        DebugInspectorError::Database(_) => {
            unreachable!("database errors do not occur while creating the debug tracer")
        }
    }
}
