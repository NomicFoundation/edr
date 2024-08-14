use edr_eth::{
    block::{BlobGas, Header},
    chain_spec::L1ChainSpec,
    env::{BlobExcessGasAndPrice, BlockEnv},
    result::ExecutionResult,
    Address, HashMap, Precompile, SpecId, U256,
};
use edr_evm::{
    blockchain::{BlockchainError, SyncBlockchain},
    evm::handler::CfgEnvWithEvmWiring,
    guaranteed_dry_run,
    state::{StateError, StateOverrides, StateRefOverrider, SyncState},
    transaction, DebugContext,
};

use crate::ProviderError;

pub(super) struct RunCallArgs<'a, 'evm, DebugDataT>
where
    'a: 'evm,
{
    pub blockchain: &'a dyn SyncBlockchain<L1ChainSpec, BlockchainError<L1ChainSpec>, StateError>,
    pub header: &'a Header,
    pub state: &'a dyn SyncState<StateError>,
    pub state_overrides: &'a StateOverrides,
    pub cfg_env: CfgEnvWithEvmWiring<L1ChainSpec>,
    pub transaction: transaction::Signed,
    pub precompiles: &'a HashMap<Address, Precompile>,
    // `DebugContext` cannot be simplified further
    #[allow(clippy::type_complexity)]
    pub debug_context: Option<
        DebugContext<
            'evm,
            L1ChainSpec,
            BlockchainError<L1ChainSpec>,
            DebugDataT,
            StateRefOverrider<'a, &'evm dyn SyncState<StateError>>,
        >,
    >,
}

/// Execute a transaction as a call. Returns the gas used and the output.
pub(super) fn run_call<'a, 'evm, DebugDataT>(
    args: RunCallArgs<'a, 'evm, DebugDataT>,
) -> Result<ExecutionResult<L1ChainSpec>, ProviderError>
where
    'a: 'evm,
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

    let block = BlockEnv {
        number: U256::from(header.number),
        coinbase: header.beneficiary,
        timestamp: U256::from(header.timestamp),
        gas_limit: U256::from(header.gas_limit),
        basefee: U256::ZERO,
        difficulty: header.difficulty,
        prevrandao: if cfg_env.spec_id >= SpecId::MERGE {
            Some(header.mix_hash)
        } else {
            None
        },
        blob_excess_gas_and_price: header
            .blob_gas
            .as_ref()
            .map(|BlobGas { excess_gas, .. }| BlobExcessGasAndPrice::new(*excess_gas)),
    };

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
