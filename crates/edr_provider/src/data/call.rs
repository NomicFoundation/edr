use edr_block_header::BlockHeader;
use edr_blockchain_api::BlockHash;
use edr_database_components::DatabaseComponents;
use edr_evm::{
    blockchain::BlockchainErrorForChainSpec, config::CfgEnv, inspector::Inspector,
    precompile::PrecompileFn, result::ExecutionResult, runtime::guaranteed_dry_run_with_inspector,
    spec::ContextForChainSpec, state::WrapDatabaseRef,
};
use edr_evm_spec::{EvmTransactionValidationError, TransactionValidation};
use edr_primitives::{Address, HashMap};
use edr_state_api::{State, StateError};

use crate::{
    error::ProviderErrorForChainSpec, time::TimeSinceEpoch, ProviderError, SyncProviderSpec,
};

/// Execute a transaction as a call. Returns the gas used and the output.
pub(super) fn run_call<BlockchainT, ChainSpecT, InspectorT, StateT, TimerT>(
    blockchain: BlockchainT,
    header: &BlockHeader,
    state: StateT,
    cfg_env: CfgEnv<ChainSpecT::Hardfork>,
    transaction: ChainSpecT::SignedTransaction,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
    inspector: &mut InspectorT,
) -> Result<ExecutionResult<ChainSpecT::HaltReason>, ProviderErrorForChainSpec<ChainSpecT>>
where
    BlockchainT: BlockHash<Error = BlockchainErrorForChainSpec<ChainSpecT>>,
    ChainSpecT: SyncProviderSpec<
        TimerT,
        BlockEnv: Default,
        SignedTransaction: Default
                               + TransactionValidation<
            ValidationError: From<EvmTransactionValidationError>,
        >,
    >,
    InspectorT: Inspector<
        ContextForChainSpec<ChainSpecT, WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>>,
    >,
    StateT: State<Error = StateError>,
    TimerT: Clone + TimeSinceEpoch,
{
    // `eth_call` uses a base fee of zero to mimick geth's behavior
    let mut header = header.clone();
    header.base_fee_per_gas = header.base_fee_per_gas.map(|_| 0);

    let block = ChainSpecT::new_block_env(&header, cfg_env.spec.into());

    guaranteed_dry_run_with_inspector::<_, ChainSpecT, _, _>(
        blockchain,
        state,
        cfg_env,
        transaction,
        block,
        custom_precompiles,
        inspector,
    )
    .map_or_else(
        |error| Err(ProviderError::RunTransaction(error)),
        |result| Ok(result.result),
    )
}
