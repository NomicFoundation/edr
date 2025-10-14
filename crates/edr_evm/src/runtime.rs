use edr_blockchain_api::BlockHash;
use edr_chain_spec::{EvmSpecId, EvmTransactionValidationError, TransactionValidation};
use edr_database_components::DatabaseComponents;
use edr_evm_spec::{ChainEvmSpec, Context, Journal, TransactionError};
use edr_primitives::{Address, HashMap};
use edr_state_api::{State, StateCommit};
use revm::{precompile::PrecompileFn, Inspector};

use crate::{
    config::CfgEnv,
    precompile::OverriddenPrecompileProvider,
    result::{ExecutionResult, ExecutionResultAndState},
    state::WrapDatabaseRef,
};

/// Runs a transaction without committing the state.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
// Cannot meaningfully be simplified further
#[allow(clippy::type_complexity)]
pub fn dry_run<
    BlockchainT: BlockHash<Error: Send + std::error::Error>,
    BlockEnvT,
    EvmT: ChainEvmSpec,
    HaltReasonT,
    HardforkT,
    PrecompileProviderT: Default,
    SignedTransactionT: TransactionValidation<ValidationError: From<EvmTransactionValidationError>>,
    StateT: State<Error: Send + std::error::Error>,
>(
    blockchain: BlockchainT,
    state: StateT,
    cfg: CfgEnv<HardforkT>,
    transaction: SignedTransactionT,
    block: BlockEnvT,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
) -> Result<
    ExecutionResultAndState<HaltReasonT>,
    TransactionError<
        DatabaseComponents<BlockchainT::Error, StateT::Error>,
        <SignedTransactionT as TransactionValidation>::ValidationError,
    >,
> {
    let database = WrapDatabaseRef(DatabaseComponents { blockchain, state });

    let precompile_provider = OverriddenPrecompileProvider::with_precompiles(
        PrecompileProviderT::default(),
        custom_precompiles.clone(),
    );

    EvmT::dry_run(block, cfg, transaction, database, precompile_provider)
}

/// Runs a transaction while observing with an inspector, without committing the
/// state.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
// Cannot meaningfully be simplified further
#[allow(clippy::type_complexity)]
pub fn dry_run_with_inspector<
    BlockchainT: BlockHash<Error: Send + std::error::Error>,
    BlockEnvT,
    EvmT: ChainEvmSpec,
    HaltReasonT,
    HardforkT,
    InspectorT: Inspector<
        Context<
            BlockEnvT,
            SignedTransactionT,
            HardforkT,
            WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
            Journal<WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>>,
            EvmT::Context,
        >,
    >,
    PrecompileProviderT: Default,
    SignedTransactionT: TransactionValidation<ValidationError: From<EvmTransactionValidationError>>,
    StateT: State<Error: Send + std::error::Error>,
>(
    blockchain: BlockchainT,
    state: StateT,
    cfg: CfgEnv<HardforkT>,
    transaction: SignedTransactionT,
    block: BlockEnvT,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
    inspector: &mut InspectorT,
) -> Result<
    ExecutionResultAndState<HaltReasonT>,
    TransactionError<
        DatabaseComponents<BlockchainT::Error, StateT::Error>,
        <SignedTransactionT as TransactionValidation>::ValidationError,
    >,
> {
    let database = WrapDatabaseRef(DatabaseComponents { blockchain, state });

    let precompile_provider = OverriddenPrecompileProvider::with_precompiles(
        EvmT::PrecompileProvider::default(),
        custom_precompiles.clone(),
    );

    EvmT::dry_run_with_inspector(
        block,
        cfg,
        transaction,
        database,
        precompile_provider,
        inspector,
    )
}

/// Runs a transaction without committing the state, while disabling balance
/// checks and creating accounts for new addresses.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
// Cannot meaningfully be simplified further
#[allow(clippy::type_complexity)]
pub fn guaranteed_dry_run<
    BlockchainT: BlockHash<Error: Send + std::error::Error>,
    BlockEnvT,
    EvmT: ChainEvmSpec,
    HaltReasonT,
    HardforkT,
    PrecompileProviderT: Default,
    SignedTransactionT: TransactionValidation<ValidationError: From<EvmTransactionValidationError>>,
    StateT: State<Error: Send + std::error::Error>,
>(
    blockchain: BlockchainT,
    state: StateT,
    mut cfg: CfgEnv<HardforkT>,
    transaction: SignedTransactionT,
    block: BlockEnvT,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
) -> Result<
    ExecutionResultAndState<HaltReasonT>,
    TransactionError<
        DatabaseComponents<BlockchainT::Error, StateT::Error>,
        <SignedTransactionT as TransactionValidation>::ValidationError,
    >,
> {
    set_guarantees(&mut cfg);

    dry_run(
        blockchain,
        state,
        cfg,
        transaction,
        block,
        custom_precompiles,
    )
}

/// Runs a transaction while observing with an inspector, without committing the
/// state, while disabling balance checks and creating accounts for new
/// addresses.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
// Cannot meaningfully be simplified further
#[allow(clippy::type_complexity)]
pub fn guaranteed_dry_run_with_inspector<
    BlockchainT: BlockHash<Error: Send + std::error::Error>,
    BlockEnvT,
    EvmT: ChainEvmSpec,
    HaltReasonT,
    HardforkT,
    InspectorT: Inspector<
        Context<
            BlockEnvT,
            SignedTransactionT,
            HardforkT,
            WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
            Journal<WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>>,
            EvmT::Context,
        >,
    >,
    PrecompileProviderT: Default,
    SignedTransactionT: TransactionValidation<ValidationError: From<EvmTransactionValidationError>>,
    StateT: State<Error: Send + std::error::Error>,
>(
    blockchain: BlockchainT,
    state: StateT,
    mut cfg: CfgEnv<HardforkT>,
    transaction: SignedTransactionT,
    block: BlockEnvT,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
    inspector: &mut InspectorT,
) -> Result<
    ExecutionResultAndState<HaltReasonT>,
    TransactionError<
        DatabaseComponents<BlockchainT::Error, StateT::Error>,
        <SignedTransactionT as TransactionValidation>::ValidationError,
    >,
> {
    set_guarantees(&mut cfg);

    dry_run_with_inspector(
        blockchain,
        state,
        cfg,
        transaction,
        block,
        custom_precompiles,
        inspector,
    )
}

/// Runs a transaction, committing the state in the process.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
// Cannot meaningfully be simplified further
#[allow(clippy::type_complexity)]
pub fn run<
    BlockchainT: BlockHash<Error: Send + std::error::Error>,
    BlockEnvT,
    EvmT: ChainEvmSpec,
    HaltReasonT,
    HardforkT,
    PrecompileProviderT: Default,
    SignedTransactionT: TransactionValidation<ValidationError: From<EvmTransactionValidationError>>,
    StateT: State<Error: Send + std::error::Error> + StateCommit,
>(
    blockchain: BlockchainT,
    mut state: StateT,
    cfg: CfgEnv<HardforkT>,
    transaction: SignedTransactionT,
    block: BlockEnvT,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
) -> Result<
    ExecutionResult<HaltReasonT>,
    TransactionError<
        DatabaseComponents<BlockchainT::Error, StateT::Error>,
        <SignedTransactionT as TransactionValidation>::ValidationError,
    >,
> {
    let ExecutionResultAndState {
        result,
        state: state_diff,
    } = dry_run(
        blockchain,
        &state,
        cfg,
        transaction,
        block,
        custom_precompiles,
    )?;

    state.commit(state_diff);

    Ok(result)
}

fn set_guarantees<HardforkT: Into<EvmSpecId>>(config: &mut CfgEnv<HardforkT>) {
    config.disable_balance_check = true;
    config.disable_block_gas_limit = true;
    config.disable_nonce_check = true;
}
