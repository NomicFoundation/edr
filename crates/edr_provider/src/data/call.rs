use core::fmt::Debug;

use edr_eth::{
    block::Header,
    env::CfgEnv,
    result::{ExecutionResult, InvalidTransaction},
    transaction::TransactionValidation,
    Address, HashMap, Precompile, U256,
};
use edr_evm::{
    blockchain::{BlockchainError, SyncBlockchain},
    chain_spec::{BlockEnvConstructor as _, ChainSpec, SyncChainSpec},
    guaranteed_dry_run,
    state::{StateError, StateOverrides, StateRefOverrider, SyncState},
    DebugContext,
};

use crate::ProviderError;

pub(super) struct RunCallArgs<
    'a,
    'evm,
    ChainSpecT: ChainSpec<Transaction: TransactionValidation<ValidationError: From<InvalidTransaction>>>,
    DebugDataT,
> where
    'a: 'evm,
{
    pub blockchain: &'a dyn SyncBlockchain<ChainSpecT, BlockchainError<ChainSpecT>, StateError>,
    pub header: &'a Header,
    pub state: &'a dyn SyncState<StateError>,
    pub state_overrides: &'a StateOverrides,
    pub cfg_env: CfgEnv,
    pub hardfork: ChainSpecT::Hardfork,
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
) -> Result<ExecutionResult<ChainSpecT::HaltReason>, ProviderError<ChainSpecT>>
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
        hardfork,
        transaction,
        precompiles,
        debug_context,
    } = args;

    // `eth_call` uses a base fee of zero to mimick geth's behavior
    let mut header = header.clone();
    header.base_fee_per_gas = header.base_fee_per_gas.map(|_| U256::ZERO);

    let block = ChainSpecT::Block::new_block_env(&header, hardfork);

    let state_overrider = StateRefOverrider::new(state_overrides, state);

    guaranteed_dry_run(
        blockchain,
        state_overrider,
        cfg_env,
        hardfork,
        transaction,
        block,
        precompiles,
        debug_context,
    )
    .map_or_else(
        |error| Err(ProviderError::RunTransaction(error)),
        |result| Ok(result.result),
    )
}
