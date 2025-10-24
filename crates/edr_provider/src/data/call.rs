use edr_block_header::BlockHeader;
use edr_blockchain_api::{r#dyn::DynBlockchainError, BlockHashByNumber};
use edr_chain_spec::BlockEnvConstructor;
use edr_chain_spec_provider::ProviderChainSpec;
use edr_database_components::{DatabaseComponents, WrapDatabaseRef};
use edr_evm2::guaranteed_dry_run_with_inspector;
use edr_evm_spec::{result::ExecutionResult, CfgEnv, ContextForChainSpec, Inspector};
use edr_precompile::PrecompileFn;
use edr_primitives::{Address, HashMap};
use edr_state_api::{State, StateError};

use crate::{error::ProviderErrorForChainSpec, ProviderError};

/// Execute a transaction as a call. Returns the gas used and the output.
pub(super) fn run_call<'header, ChainSpecT, BlockchainT, InspectorT, StateT>(
    blockchain: BlockchainT,
    header: &'header BlockHeader,
    state: StateT,
    cfg_env: CfgEnv<ChainSpecT::Hardfork>,
    transaction: ChainSpecT::SignedTransaction,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
    inspector: &mut InspectorT,
) -> Result<ExecutionResult<ChainSpecT::HaltReason>, ProviderErrorForChainSpec<ChainSpecT>>
where
    BlockchainT: BlockHashByNumber<Error = DynBlockchainError>,
    ChainSpecT: ProviderChainSpec,
    InspectorT: Inspector<
        ContextForChainSpec<
            ChainSpecT,
            ChainSpecT::BlockEnv<'header, BlockHeader>,
            WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
        >,
    >,
    StateT: State<Error = StateError>,
{
    // `eth_call` uses a base fee of zero to mimick geth's behavior
    let mut header = header.clone();
    header.base_fee_per_gas = header.base_fee_per_gas.map(|_| 0);

    let block = ChainSpecT::BlockEnv::new_block_env(&header, cfg_env.spec);

    guaranteed_dry_run_with_inspector::<ChainSpecT, _, _, _, _>(
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
