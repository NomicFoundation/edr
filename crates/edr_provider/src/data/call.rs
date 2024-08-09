use core::fmt::Debug;

use edr_eth::{
    block::Header,
    result::{ExecutionResult, InvalidTransaction},
    transaction::TransactionValidation,
    Address, HashMap, Precompile,
};
use edr_evm::{
    blockchain::{BlockchainError, SyncBlockchain},
    chain_spec::BlockEnvConstructor as _,
    chain_spec::{ChainSpec, SyncChainSpec},
    evm::handler::CfgEnvWithEvmWiring,
    guaranteed_dry_run,
    state::{StateError, StateOverrides, StateRefOverrider, SyncState},
    DebugContext,
};

use crate::ProviderError;

pub(super) struct RunCallArgs<'a, 'evm, ChainSpecT: ChainSpec, DebugDataT>
where
    'a: 'evm,
{
    pub blockchain: &'a dyn SyncBlockchain<ChainSpecT, BlockchainError<ChainSpecT>, StateError>,
    pub header: &'a Header,
    pub state: &'a dyn SyncState<StateError>,
    pub state_overrides: &'a StateOverrides,
    pub cfg_env: CfgEnvWithEvmWiring<ChainSpecT>,
    pub transaction: ChainSpecT::Transaction,
    pub precompiles: &'a HashMap<Address, Precompile>,
    // `DebugContext` cannot be simplified further
    #[allow(clippy::type_complexity)]
    pub debug_context: Option<
        DebugContext<
            'evm,
            ChainSpecT,
            BlockchainError<ChainSpecT>,
            DebugDataT,
            StateRefOverrider<'a, &'evm dyn SyncState<StateError>>,
        >,
    >,
}

/// Execute a transaction as a call. Returns the gas used and the output.
pub(super) fn run_call<'a, 'evm, ChainSpecT, DebugDataT>(
    args: RunCallArgs<'a, 'evm, ChainSpecT, DebugDataT>,
) -> Result<ExecutionResult<ChainSpecT>, ProviderError<ChainSpecT>>
where
    'a: 'evm,
    ChainSpecT: SyncChainSpec<
        Block: Default,
        Hardfork: Debug,
        Transaction: Default + TransactionValidation<ValidationError: From<InvalidTransaction>>,
    >,
{
    let RunCallArgs {
        blockchain,
        header,
        state,
        state_overrides,
        cfg_env,
        transaction: tx_env,
        precompiles,
        debug_context,
    } = args;

    let block = ChainSpecT::Block::new_block_env(header, cfg_env.spec_id);

    guaranteed_dry_run(
        blockchain,
        state,
        state_overrides,
        cfg_env,
        tx_env,
        block,
        precompiles,
        debug_context,
    )
    .map_or_else(
        |error| Err(ProviderError::RunTransaction(error)),
        |result| Ok(result.result),
    )
}
