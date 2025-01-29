use edr_eth::{
    block::Header, l1, result::ExecutionResult, transaction::TransactionValidation, Address,
    HashMap, U256,
};
use edr_evm::{
    blockchain::{BlockchainErrorForChainSpec, SyncBlockchain},
    config::CfgEnv,
    guaranteed_dry_run,
    spec::{BlockEnvConstructor as _, RuntimeSpec, SyncRuntimeSpec},
    state::{StateError, StateOverrides, StateRefOverrider, SyncState},
    ContextExtension,
};
use revm_precompile::PrecompileFn;

use crate::ProviderError;

pub(super) struct RunCallArgs<
    'args,
    'extension,
    ChainSpecT: RuntimeSpec<
        SignedTransaction: TransactionValidation<ValidationError: From<l1::InvalidTransaction>>,
    >,
    ExtensionT,
    FrameT,
> where
    'args: 'extension,
{
    pub blockchain:
        &'args dyn SyncBlockchain<ChainSpecT, BlockchainErrorForChainSpec<ChainSpecT>, StateError>,
    pub header: &'args Header,
    pub state: &'args dyn SyncState<StateError>,
    pub state_overrides: &'args StateOverrides,
    pub cfg_env: CfgEnv<ChainSpecT::Hardfork>,
    pub transaction: ChainSpecT::SignedTransaction,
    pub precompiles: &'args HashMap<Address, PrecompileFn>,
    // `DebugContext` cannot be simplified further
    #[allow(clippy::type_complexity)]
    pub debug_context: Option<&'extension mut ContextExtension<ExtensionT, FrameT>>,
}

/// Execute a transaction as a call. Returns the gas used and the output.
pub(super) fn run_call<'args, 'extension, ChainSpecT, ExtensionT, FrameT>(
    args: RunCallArgs<'args, 'extension, ChainSpecT, ExtensionT, FrameT>,
) -> Result<ExecutionResult<ChainSpecT::HaltReason>, ProviderError<ChainSpecT>>
where
    'args: 'extension,
    ChainSpecT: SyncRuntimeSpec<
        BlockEnv: Default,
        SignedTransaction: Default
                               + TransactionValidation<ValidationError: From<l1::InvalidTransaction>>,
    >,
{
    let RunCallArgs {
        blockchain,
        header,
        state,
        state_overrides,
        cfg_env,
        transaction,
        precompiles,
        debug_context,
    } = args;

    // `eth_call` uses a base fee of zero to mimick geth's behavior
    let mut header = header.clone();
    header.base_fee_per_gas = header.base_fee_per_gas.map(|_| 0);

    let block = ChainSpecT::BlockEnv::new_block_env(&header, hardfork.into());

    let state_overrider = StateRefOverrider::new(state_overrides, state);

    guaranteed_dry_run_with(
        blockchain,
        state_overrider,
        cfg_env,
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
