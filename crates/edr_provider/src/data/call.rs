use edr_eth::{
    block::Header, result::ExecutionResult, transaction::TransactionValidation, Address, HashMap,
};
use edr_evm::{
    blockchain::{BlockHash, BlockchainErrorForChainSpec},
    config::CfgEnv,
    inspector::Inspector,
    precompile::PrecompileFn,
    runtime::guaranteed_dry_run_with_inspector,
    spec::{BlockEnvConstructor as _, ContextForChainSpec, SyncRuntimeSpec},
    state::{DatabaseComponents, State, StateError, WrapDatabaseRef},
    EvmInvalidTransaction,
};

use crate::{error::ProviderErrorForChainSpec, ProviderError};

/// Execute a transaction as a call. Returns the gas used and the output.
pub(super) fn run_call<BlockchainT, ChainSpecT, InspectorT, StateT>(
    blockchain: BlockchainT,
    header: &Header,
    state: StateT,
    cfg_env: CfgEnv<ChainSpecT::Hardfork>,
    transaction: ChainSpecT::SignedTransaction,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
    inspector: &mut InspectorT,
) -> Result<ExecutionResult<ChainSpecT::HaltReason>, ProviderErrorForChainSpec<ChainSpecT>>
where
    BlockchainT: BlockHash<Error = BlockchainErrorForChainSpec<ChainSpecT>>,
    ChainSpecT: SyncRuntimeSpec<
        BlockEnv: Default,
        SignedTransaction: Default
                               + TransactionValidation<ValidationError: From<EvmInvalidTransaction>>,
    >,
    InspectorT: Inspector<
        ContextForChainSpec<ChainSpecT, WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>>,
    >,
    StateT: State<Error = StateError>,
{
    // `eth_call` uses a base fee of zero to mimick geth's behavior
    let mut header = header.clone();
    header.base_fee_per_gas = header.base_fee_per_gas.map(|_| 0);

    let block = ChainSpecT::BlockEnv::new_block_env(&header, cfg_env.spec.into());

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
