use edr_block_header::BlockHeader;
use edr_blockchain_api::{r#dyn::DynBlockchainError, BlockHashByNumber};
use edr_chain_spec_evm::CfgEnv;
use edr_chain_spec_provider::ProviderChainSpec;
use edr_evm::{set_guarantees, ExecutionResultWithMetadata};
use edr_inspection::{BlockEnvOverrides, InspectedExecution, InspectionError, InspectionRequest};
use edr_precompile::PrecompileFn;
use edr_primitives::{Address, HashMap};
use edr_state_api::{State, StateError};

use crate::{
    error::ProviderErrorForChainSpec,
    observability::{EvmObserverConfig, ObservedExecution},
    ProviderError,
};

/// Executes a transaction as a call, observed as described by
/// `observer_config`.
pub(super) fn run_call<'call, ChainSpecT, BlockchainT, StateT>(
    blockchain: BlockchainT,
    block_env: ChainSpecT::BlockEnv<'call, BlockHeader>,
    state: StateT,
    mut cfg_env: CfgEnv<ChainSpecT::ProtocolHardfork>,
    transaction: ChainSpecT::SignedTransaction,
    custom_precompiles: &'call HashMap<Address, PrecompileFn>,
    observer_config: &EvmObserverConfig,
) -> Result<
    ObservedExecution<ExecutionResultWithMetadata<ChainSpecT::HaltReason>>,
    ProviderErrorForChainSpec<ChainSpecT>,
>
where
    BlockchainT: BlockHashByNumber<Error = DynBlockchainError>,
    ChainSpecT: ProviderChainSpec,
    StateT: State<Error = StateError>,
{
    set_guarantees(&mut cfg_env);

    let request = InspectionRequest {
        observer: observer_config.clone(),
        debug_trace: None,
    };

    let InspectedExecution {
        execution,
        observed,
        geth_trace: _,
    } = ChainSpecT::dry_run_with_inspection(
        &blockchain,
        &state,
        cfg_env,
        transaction,
        &block_env,
        BlockEnvOverrides {
            // `eth_call` uses a base fee of zero to mimick geth's behavior
            base_fee: Some(0),
        },
        custom_precompiles,
        request,
    )
    .map_err(|error| match error {
        InspectionError::Transaction(error) => ProviderError::RunTransaction(error),
        InspectionError::Collection(error) => ProviderError::from(error),
        InspectionError::Database(_) | InspectionError::DebugTracer(_) => {
            unreachable!("only the debug tracer fails with these, and none was requested")
        }
    })?;

    Ok(ObservedExecution {
        evm_observed_data: observed,
        execution_result: execution.into_result_with_metadata(),
    })
}
