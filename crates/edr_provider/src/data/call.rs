use edr_eth::{block::Header, l1, result::ExecutionResult, transaction::TransactionValidation};
use edr_evm::{
    blockchain::{BlockHash, BlockchainErrorForChainSpec},
    config::CfgEnv,
    inspector::Inspector,
    runtime::guaranteed_dry_run_with_inspector,
    spec::{BlockEnvConstructor as _, ContextForChainSpec, SyncRuntimeSpec},
    state::{DatabaseComponents, State, StateError, WrapDatabaseRef},
};

use crate::{error::ProviderErrorForChainSpec, ProviderError};

/// Execute a transaction as a call. Returns the gas used and the output.
pub(super) fn run_call<
    'components,
    'context,
    'inspector,
    BlockchainT,
    ChainSpecT,
    InspectorT,
    StateT,
>(
    blockchain: BlockchainT,
    header: &Header,
    state: StateT,
    cfg_env: CfgEnv<ChainSpecT::Hardfork>,
    transaction: ChainSpecT::SignedTransaction,
    inspector: &mut InspectorT,
) -> Result<ExecutionResult<ChainSpecT::HaltReason>, ProviderErrorForChainSpec<ChainSpecT>>
where
    'components: 'context,
    'inspector: 'context,
    BlockchainT: BlockHash<Error = BlockchainErrorForChainSpec<ChainSpecT>> + 'components,
    ChainSpecT: SyncRuntimeSpec<
        BlockEnv: Default,
        SignedTransaction: Default
                               + TransactionValidation<ValidationError: From<l1::InvalidTransaction>>,
    >,
    InspectorT: Inspector<
        ContextForChainSpec<ChainSpecT, WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>>,
    >,
    StateT: State<Error = StateError> + 'components,
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
        inspector,
    )
    .map_or_else(
        |error| Err(ProviderError::RunTransaction(error)),
        |result| Ok(result.result),
    )
}
