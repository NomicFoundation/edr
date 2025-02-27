use core::fmt::Debug;

use edr_eth::{
    block::{BlobGas, Header},
    Address, HashMap, SpecId, U256,
};
use edr_evm::{
    blockchain::{BlockchainError, SyncBlockchain},
    chain_spec::L1ChainSpec,
    guaranteed_dry_run,
    state::{StateError, StateOverrides, StateRefOverrider, SyncState},
    BlobExcessGasAndPrice, BlockEnv, CfgEnvWithHandlerCfg, DebugContext, ExecutionResult,
    Precompile, TxEnv,
};

use crate::ProviderError;

pub(super) struct RunCallArgs<'a, 'evm, DebugDataT>
where
    'a: 'evm,
{
    pub blockchain: &'a dyn SyncBlockchain<L1ChainSpec, BlockchainError, StateError>,
    pub header: &'a Header,
    pub state: &'a dyn SyncState<StateError>,
    pub state_overrides: &'a StateOverrides,
    pub cfg_env: CfgEnvWithHandlerCfg,
    pub tx_env: TxEnv,
    pub precompiles: &'a HashMap<Address, Precompile>,
    // `DebugContext` cannot be simplified further
    #[allow(clippy::type_complexity)]
    pub debug_context: Option<
        DebugContext<
            'evm,
            L1ChainSpec,
            BlockchainError,
            DebugDataT,
            StateRefOverrider<'a, &'evm dyn SyncState<StateError>>,
        >,
    >,
}

/// Execute a transaction as a call. Returns the gas used and the output.
pub(super) fn run_call<'a, 'evm, DebugDataT, LoggerErrorT: Debug>(
    args: RunCallArgs<'a, 'evm, DebugDataT>,
) -> Result<ExecutionResult, ProviderError<LoggerErrorT>>
where
    'a: 'evm,
{
    let RunCallArgs {
        blockchain,
        header,
        state,
        state_overrides,
        cfg_env,
        tx_env,
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
        prevrandao: if cfg_env.handler_cfg.spec_id >= SpecId::MERGE {
            Some(header.mix_hash)
        } else {
            None
        },
        blob_excess_gas_and_price: header.blob_gas.as_ref().map(
            |BlobGas { excess_gas, .. }| {
                BlobExcessGasAndPrice::new(
                    *excess_gas,
                    cfg_env.handler_cfg.spec_id >= SpecId::PRAGUE,
                )
            },
        ),
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
