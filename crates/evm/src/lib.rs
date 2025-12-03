//! Utilities for running transactions in the EVM.
#![warn(missing_docs)]

use edr_blockchain_api::BlockHashByNumber;
use edr_chain_spec::{EvmSpecId, TransactionValidation};
use edr_chain_spec_evm::{
    result::{ExecutionResult, ExecutionResultAndState},
    BlockEnvTrait, CfgEnv, ContextForChainSpec, DatabaseComponentError, EvmChainSpec, Inspector,
    TransactionError,
};
use edr_database_components::{DatabaseComponents, WrapDatabaseRef};
use edr_precompile::{OverriddenPrecompileProvider, PrecompileFn};
use edr_primitives::{Address, HashMap};
use edr_state_api::{State, StateCommit};

/// Runs a transaction without committing the state.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
// Cannot meaningfully be simplified further
#[allow(clippy::type_complexity)]
pub fn dry_run<
    // As this generic type always needs to be specified, placing it first makes the function
    // easier to use; e.g.
    // ```
    // dry_run::<MyChainSpec, _, _>(...)
    // ```
    EvmChainSpecT: EvmChainSpec,
    BlockT: BlockEnvTrait,
    BlockchainT: BlockHashByNumber<Error: std::error::Error>,
    StateT: State<Error: std::error::Error>,
>(
    blockchain: BlockchainT,
    state: StateT,
    cfg: CfgEnv<EvmChainSpecT::Hardfork>,
    transaction: EvmChainSpecT::SignedTransaction,
    block: BlockT,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
) -> Result<
    ExecutionResultAndState<EvmChainSpecT::HaltReason>,
    TransactionError<
        DatabaseComponentError<BlockchainT::Error, StateT::Error>,
        <EvmChainSpecT::SignedTransaction as TransactionValidation>::ValidationError,
    >,
> {
    let database = WrapDatabaseRef(DatabaseComponents { blockchain, state });

    let precompile_provider = OverriddenPrecompileProvider::with_precompiles(
        EvmChainSpecT::PrecompileProvider::default(),
        custom_precompiles.clone(),
    );

    EvmChainSpecT::dry_run(block, cfg, transaction, database, precompile_provider)
}

/// Runs a transaction while observing with an inspector, without committing the
/// state.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
// Cannot meaningfully be simplified further
#[allow(clippy::type_complexity)]
pub fn dry_run_with_inspector<
    // As this generic type always needs to be specified, placing it first makes the function
    // easier to use; e.g.
    // ```
    // dry_run::<MyChainSpec, _, _, _>(...)
    // ```
    EvmChainSpecT: EvmChainSpec,
    BlockT: BlockEnvTrait,
    BlockchainT: BlockHashByNumber<Error: std::error::Error>,
    InspectorT: Inspector<
        ContextForChainSpec<
            EvmChainSpecT,
            BlockT,
            WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
        >,
    >,
    StateT: State<Error: std::error::Error>,
>(
    blockchain: BlockchainT,
    state: StateT,
    cfg: CfgEnv<EvmChainSpecT::Hardfork>,
    transaction: EvmChainSpecT::SignedTransaction,
    block: BlockT,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
    inspector: &mut InspectorT,
) -> Result<
    ExecutionResultAndState<EvmChainSpecT::HaltReason>,
    TransactionError<
        DatabaseComponentError<BlockchainT::Error, StateT::Error>,
        <EvmChainSpecT::SignedTransaction as TransactionValidation>::ValidationError,
    >,
> {
    let database = WrapDatabaseRef(DatabaseComponents { blockchain, state });

    let precompile_provider = OverriddenPrecompileProvider::with_precompiles(
        EvmChainSpecT::PrecompileProvider::default(),
        custom_precompiles.clone(),
    );

    EvmChainSpecT::dry_run_with_inspector(
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
    // As this generic type always needs to be specified, placing it first makes the function
    // easier to use; e.g.
    // ```
    // dry_run::<MyChainSpec, _, _>(...)
    // ```
    EvmChainSpecT: EvmChainSpec,
    BlockT: BlockEnvTrait,
    BlockchainT: BlockHashByNumber<Error: std::error::Error>,
    StateT: State<Error: std::error::Error>,
>(
    blockchain: BlockchainT,
    state: StateT,
    mut cfg: CfgEnv<EvmChainSpecT::Hardfork>,
    transaction: EvmChainSpecT::SignedTransaction,
    block: BlockT,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
) -> Result<
    ExecutionResultAndState<EvmChainSpecT::HaltReason>,
    TransactionError<
        DatabaseComponentError<BlockchainT::Error, StateT::Error>,
        <EvmChainSpecT::SignedTransaction as TransactionValidation>::ValidationError,
    >,
> {
    set_guarantees(&mut cfg);

    dry_run::<EvmChainSpecT, _, _, _>(
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
    // As this generic type always needs to be specified, placing it first makes the function
    // easier to use; e.g.
    // ```
    // dry_run::<MyChainSpec, _, _, _>(...)
    // ```
    EvmChainSpecT: EvmChainSpec,
    BlockT: BlockEnvTrait,
    BlockchainT: BlockHashByNumber<Error: std::error::Error>,
    InspectorT: Inspector<
        ContextForChainSpec<
            EvmChainSpecT,
            BlockT,
            WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
        >,
    >,
    StateT: State<Error: std::error::Error>,
>(
    blockchain: BlockchainT,
    state: StateT,
    mut cfg: CfgEnv<EvmChainSpecT::Hardfork>,
    transaction: EvmChainSpecT::SignedTransaction,
    block: BlockT,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
    inspector: &mut InspectorT,
) -> Result<
    ExecutionResultAndState<EvmChainSpecT::HaltReason>,
    TransactionError<
        DatabaseComponentError<BlockchainT::Error, StateT::Error>,
        <EvmChainSpecT::SignedTransaction as TransactionValidation>::ValidationError,
    >,
> {
    set_guarantees(&mut cfg);

    dry_run_with_inspector::<EvmChainSpecT, _, _, _, _>(
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
    // As this generic type always needs to be specified, placing it first makes the function
    // easier to use; e.g.
    // ```
    // dry_run::<MyChainSpec, _, _>(...)
    // ```
    EvmChainSpecT: EvmChainSpec,
    BlockT: BlockEnvTrait,
    BlockchainT: BlockHashByNumber<Error: std::error::Error>,
    StateT: State<Error: std::error::Error> + StateCommit,
>(
    blockchain: BlockchainT,
    mut state: StateT,
    cfg: CfgEnv<EvmChainSpecT::Hardfork>,
    transaction: EvmChainSpecT::SignedTransaction,
    block: BlockT,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
) -> Result<
    ExecutionResult<EvmChainSpecT::HaltReason>,
    TransactionError<
        DatabaseComponentError<BlockchainT::Error, StateT::Error>,
        <EvmChainSpecT::SignedTransaction as TransactionValidation>::ValidationError,
    >,
> {
    let ExecutionResultAndState {
        result,
        state: state_diff,
    } = dry_run::<EvmChainSpecT, _, _, _>(
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
