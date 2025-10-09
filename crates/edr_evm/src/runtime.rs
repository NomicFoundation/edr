use edr_blockchain_api::BlockHash;
use edr_database_components::DatabaseComponents;
use edr_evm_spec::{EvmSpecId, EvmTransactionValidationError, TransactionValidation};
use edr_primitives::{Address, HashMap};
use edr_state_api::{State, StateCommit};
use revm::{precompile::PrecompileFn, ExecuteEvm, InspectEvm, Inspector};

use crate::{
    config::CfgEnv,
    precompile::OverriddenPrecompileProvider,
    result::{EVMError, ExecutionResult, ExecutionResultAndState},
    spec::{ContextForChainSpec, RuntimeSpec},
    state::WrapDatabaseRef,
    transaction::{TransactionError, TransactionErrorForChainSpec},
};

/// Runs a transaction without committing the state.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
// Cannot meaningfully be simplified further
#[allow(clippy::type_complexity)]
pub fn dry_run<BlockchainT, ChainSpecT, StateT>(
    blockchain: BlockchainT,
    state: StateT,
    cfg: CfgEnv<ChainSpecT::Hardfork>,
    transaction: ChainSpecT::SignedTransaction,
    block: ChainSpecT::BlockEnv,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
) -> Result<
    ExecutionResultAndState<ChainSpecT::HaltReason>,
    TransactionErrorForChainSpec<BlockchainT::Error, ChainSpecT, StateT::Error>,
>
where
    BlockchainT: BlockHash<Error: Send + std::error::Error>,
    ChainSpecT: RuntimeSpec<
        SignedTransaction: TransactionValidation<
            ValidationError: From<EvmTransactionValidationError>,
        >,
    >,
    StateT: State<Error: Send + std::error::Error>,
{
    let database = WrapDatabaseRef(DatabaseComponents { blockchain, state });

    let precompile_provider = OverriddenPrecompileProvider::with_precompiles(
        ChainSpecT::PrecompileProvider::default(),
        custom_precompiles.clone(),
    );

    let mut evm = ChainSpecT::evm(block, cfg, transaction, database, precompile_provider)?;
    evm.replay().map_err(|error| match error {
        EVMError::Transaction(error) => ChainSpecT::cast_transaction_error(error),
        EVMError::Header(error) => TransactionError::InvalidHeader(error),
        EVMError::Database(error) => error.into(),
        EVMError::Custom(error) => TransactionError::Custom(error),
    })
}

/// Runs a transaction while observing with an inspector, without committing the
/// state.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
// Cannot meaningfully be simplified further
#[allow(clippy::type_complexity)]
pub fn dry_run_with_inspector<BlockchainT, ChainSpecT, InspectorT, StateT>(
    blockchain: BlockchainT,
    state: StateT,
    cfg: CfgEnv<ChainSpecT::Hardfork>,
    transaction: ChainSpecT::SignedTransaction,
    block: ChainSpecT::BlockEnv,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
    inspector: &mut InspectorT,
) -> Result<
    ExecutionResultAndState<ChainSpecT::HaltReason>,
    TransactionErrorForChainSpec<BlockchainT::Error, ChainSpecT, StateT::Error>,
>
where
    BlockchainT: BlockHash<Error: Send + std::error::Error>,
    ChainSpecT: RuntimeSpec<
        SignedTransaction: TransactionValidation<
            ValidationError: From<EvmTransactionValidationError>,
        >,
    >,
    StateT: State<Error: Send + std::error::Error>,
    InspectorT: Inspector<
        ContextForChainSpec<ChainSpecT, WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>>,
    >,
{
    let database = WrapDatabaseRef(DatabaseComponents { blockchain, state });

    let precompile_provider = OverriddenPrecompileProvider::with_precompiles(
        ChainSpecT::PrecompileProvider::default(),
        custom_precompiles.clone(),
    );

    let mut evm = ChainSpecT::evm_with_inspector(
        block,
        cfg,
        // We need to pass a transaction here to properly initialize the context.
        // This default transaction is immediately overridden by the actual transaction passed to
        // `InspectEvm::inspect_tx`, so its values do not affect the inspection process.
        ChainSpecT::SignedTransaction::default(),
        database,
        inspector,
        precompile_provider,
    )?;
    evm.inspect_tx(transaction).map_err(|error| match error {
        EVMError::Transaction(error) => ChainSpecT::cast_transaction_error(error),
        EVMError::Header(error) => TransactionError::InvalidHeader(error),
        EVMError::Database(error) => error.into(),
        EVMError::Custom(error) => TransactionError::Custom(error),
    })
}

/// Runs a transaction without committing the state, while disabling balance
/// checks and creating accounts for new addresses.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
// Cannot meaningfully be simplified further
#[allow(clippy::type_complexity)]
pub fn guaranteed_dry_run<BlockchainT, ChainSpecT, StateT>(
    blockchain: BlockchainT,
    state: StateT,
    mut cfg: CfgEnv<ChainSpecT::Hardfork>,
    transaction: ChainSpecT::SignedTransaction,
    block: ChainSpecT::BlockEnv,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
) -> Result<
    ExecutionResultAndState<ChainSpecT::HaltReason>,
    TransactionErrorForChainSpec<BlockchainT::Error, ChainSpecT, StateT::Error>,
>
where
    BlockchainT: BlockHash<Error: Send + std::error::Error>,
    ChainSpecT: RuntimeSpec<
        SignedTransaction: TransactionValidation<
            ValidationError: From<EvmTransactionValidationError>,
        >,
    >,
    StateT: State<Error: Send + std::error::Error>,
{
    set_guarantees(&mut cfg);

    dry_run::<_, ChainSpecT, _>(
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
pub fn guaranteed_dry_run_with_inspector<BlockchainT, ChainSpecT, InspectorT, StateT>(
    blockchain: BlockchainT,
    state: StateT,
    mut cfg: CfgEnv<ChainSpecT::Hardfork>,
    transaction: ChainSpecT::SignedTransaction,
    block: ChainSpecT::BlockEnv,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
    inspector: &mut InspectorT,
) -> Result<
    ExecutionResultAndState<ChainSpecT::HaltReason>,
    TransactionErrorForChainSpec<BlockchainT::Error, ChainSpecT, StateT::Error>,
>
where
    BlockchainT: BlockHash<Error: Send + std::error::Error>,
    ChainSpecT: RuntimeSpec<
        SignedTransaction: TransactionValidation<
            ValidationError: From<EvmTransactionValidationError>,
        >,
    >,
    InspectorT: Inspector<
        ContextForChainSpec<ChainSpecT, WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>>,
    >,
    StateT: State<Error: Send + std::error::Error>,
{
    set_guarantees(&mut cfg);

    dry_run_with_inspector::<_, ChainSpecT, _, _>(
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
pub fn run<BlockchainT, ChainSpecT, StateT>(
    blockchain: BlockchainT,
    mut state: StateT,
    cfg: CfgEnv<ChainSpecT::Hardfork>,
    transaction: ChainSpecT::SignedTransaction,
    block: ChainSpecT::BlockEnv,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
) -> Result<
    ExecutionResult<ChainSpecT::HaltReason>,
    TransactionErrorForChainSpec<BlockchainT::Error, ChainSpecT, StateT::Error>,
>
where
    BlockchainT: BlockHash<Error: Send + std::error::Error>,
    ChainSpecT: RuntimeSpec<
        SignedTransaction: TransactionValidation<
            ValidationError: From<EvmTransactionValidationError>,
        >,
    >,
    StateT: State<Error: Send + std::error::Error> + StateCommit,
{
    let ExecutionResultAndState {
        result,
        state: state_diff,
    } = dry_run::<_, ChainSpecT, _>(
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
