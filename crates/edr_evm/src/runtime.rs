use std::fmt::Debug;

use edr_eth::{
    db::{DatabaseComponents, StateRef},
    env::{CfgEnv, Env},
    result::{ExecutionResult, InvalidTransaction, ResultAndState},
    transaction::{ExecutableTransaction as _, TransactionValidation},
    Address, HashMap, Precompile, SpecId,
};
use revm::{db::WrapDatabaseRef, ContextPrecompile, DatabaseCommit, Evm};

use crate::{
    blockchain::SyncBlockchain, chain_spec::RuntimeSpec, debug::DebugContext,
    precompiles::register_precompiles_handles, transaction::TransactionError,
};

/// Asynchronous implementation of the Database super-trait
pub type SyncDatabase<'blockchain, 'state, ChainSpecT, BlockchainErrorT, StateErrorT> =
    DatabaseComponents<
        &'state dyn StateRef<Error = StateErrorT>,
        &'blockchain dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateErrorT>,
    >;

/// Runs a transaction without committing the state.
// `DebugContext` cannot be simplified further
#[allow(clippy::too_many_arguments)]
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub fn dry_run<'blockchain, 'evm, ChainSpecT, DebugDataT, BlockchainErrorT, StateT>(
    blockchain: &'blockchain dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateT::Error>,
    state: StateT,
    cfg: CfgEnv,
    hardfork: ChainSpecT::Hardfork,
    transaction: ChainSpecT::Transaction,
    block: ChainSpecT::Block,
    custom_precompiles: &HashMap<Address, Precompile>,
    debug_context: Option<DebugContext<'evm, ChainSpecT, BlockchainErrorT, DebugDataT, StateT>>,
) -> Result<
    ResultAndState<ChainSpecT::HaltReason>,
    TransactionError<ChainSpecT, BlockchainErrorT, StateT::Error>,
>
where
    'blockchain: 'evm,
    ChainSpecT:
        RuntimeSpec<Transaction: TransactionValidation<ValidationError: From<InvalidTransaction>>>,
    BlockchainErrorT: Debug + Send,
    StateT: StateRef<Error: Debug + Send>,
{
    validate_configuration::<ChainSpecT, BlockchainErrorT, StateT::Error>(hardfork, &transaction)?;

    let env = Env::boxed(cfg, block, transaction);
    let result = {
        if let Some(debug_context) = debug_context {
            let precompiles: HashMap<Address, ContextPrecompile<_>> = custom_precompiles
                .iter()
                .map(|(address, precompile)| {
                    (*address, ContextPrecompile::from(precompile.clone()))
                })
                .collect();

            let mut evm = Evm::<ChainSpecT::EvmWiring<_, _>>::builder()
                .with_db(WrapDatabaseRef(DatabaseComponents {
                    state,
                    block_hash: blockchain,
                }))
                .with_external_context(debug_context.data)
                .with_env(env)
                .with_spec_id(hardfork)
                .append_handler_register(debug_context.register_handles_fn)
                .append_handler_register_box(Box::new(move |handler| {
                    register_precompiles_handles(handler, precompiles.clone());
                }))
                .build();

            evm.transact()
        } else {
            let precompiles: HashMap<Address, ContextPrecompile<_>> = custom_precompiles
                .iter()
                .map(|(address, precompile)| {
                    (*address, ContextPrecompile::from(precompile.clone()))
                })
                .collect();

            let mut evm = Evm::<ChainSpecT::EvmWiring<_, _>>::builder()
                .with_db(WrapDatabaseRef(DatabaseComponents {
                    state,
                    block_hash: blockchain,
                }))
                .with_external_context(())
                .with_env(env)
                .with_spec_id(hardfork)
                .append_handler_register_box(Box::new(move |handler| {
                    register_precompiles_handles(handler, precompiles.clone());
                }))
                .build();

            evm.transact()
        }
    };

    result.map_err(TransactionError::from)
}

/// Runs a transaction without committing the state, while disabling balance
/// checks and creating accounts for new addresses.
// `DebugContext` cannot be simplified further
#[allow(clippy::too_many_arguments)]
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub fn guaranteed_dry_run<
    'blockchain,
    'evm,
    'state,
    ChainSpecT,
    DebugDataT,
    BlockchainErrorT,
    StateT,
>(
    blockchain: &'blockchain dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateT::Error>,
    state: StateT,
    mut cfg: CfgEnv,
    hardfork: ChainSpecT::Hardfork,
    transaction: ChainSpecT::Transaction,
    block: ChainSpecT::Block,
    custom_precompiles: &HashMap<Address, Precompile>,
    debug_context: Option<DebugContext<'evm, ChainSpecT, BlockchainErrorT, DebugDataT, StateT>>,
) -> Result<
    ResultAndState<ChainSpecT::HaltReason>,
    TransactionError<ChainSpecT, BlockchainErrorT, StateT::Error>,
>
where
    'blockchain: 'evm,
    'state: 'evm,
    ChainSpecT:
        RuntimeSpec<Transaction: TransactionValidation<ValidationError: From<InvalidTransaction>>>,
    BlockchainErrorT: Debug + Send,
    StateT: StateRef<Error: Debug + Send>,
{
    cfg.disable_balance_check = true;
    cfg.disable_block_gas_limit = true;
    cfg.disable_nonce_check = true;
    dry_run(
        blockchain,
        state,
        cfg,
        hardfork,
        transaction,
        block,
        custom_precompiles,
        debug_context,
    )
}

/// Runs a transaction, committing the state in the process.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
#[allow(clippy::too_many_arguments)]
pub fn run<'blockchain, 'evm, ChainSpecT, BlockchainErrorT, DebugDataT, StateT>(
    blockchain: &'blockchain dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateT::Error>,
    state: StateT,
    cfg: CfgEnv,
    hardfork: ChainSpecT::Hardfork,
    transaction: ChainSpecT::Transaction,
    block: ChainSpecT::Block,
    custom_precompiles: &HashMap<Address, Precompile>,
    debug_context: Option<DebugContext<'evm, ChainSpecT, BlockchainErrorT, DebugDataT, StateT>>,
) -> Result<
    ExecutionResult<ChainSpecT::HaltReason>,
    TransactionError<ChainSpecT, BlockchainErrorT, StateT::Error>,
>
where
    'blockchain: 'evm,
    ChainSpecT:
        RuntimeSpec<Transaction: TransactionValidation<ValidationError: From<InvalidTransaction>>>,
    BlockchainErrorT: Debug + Send,
    StateT: StateRef + DatabaseCommit,
    StateT::Error: Debug + Send,
{
    validate_configuration::<ChainSpecT, BlockchainErrorT, StateT::Error>(hardfork, &transaction)?;

    let env = Env::boxed(cfg, block, transaction);

    let result = if let Some(debug_context) = debug_context {
        let precompiles: HashMap<Address, ContextPrecompile<ChainSpecT::EvmWiring<_, _>>> =
            custom_precompiles
                .iter()
                .map(|(address, precompile)| {
                    (*address, ContextPrecompile::from(precompile.clone()))
                })
                .collect();

        let mut evm = Evm::<ChainSpecT::EvmWiring<_, _>>::builder()
            .with_db(WrapDatabaseRef(DatabaseComponents {
                state,
                block_hash: blockchain,
            }))
            .with_external_context(debug_context.data)
            .with_env(env)
            .with_spec_id(hardfork)
            .append_handler_register(debug_context.register_handles_fn)
            .append_handler_register_box(Box::new(move |handler| {
                register_precompiles_handles(handler, precompiles.clone());
            }))
            .build();

        evm.transact_commit()
    } else {
        let precompiles: HashMap<Address, ContextPrecompile<ChainSpecT::EvmWiring<_, _>>> =
            custom_precompiles
                .iter()
                .map(|(address, precompile)| {
                    (*address, ContextPrecompile::from(precompile.clone()))
                })
                .collect();

        let mut evm = Evm::<ChainSpecT::EvmWiring<_, _>>::builder()
            .with_db(WrapDatabaseRef(DatabaseComponents {
                state,
                block_hash: blockchain,
            }))
            .with_external_context(())
            .with_env(env)
            .with_spec_id(hardfork)
            .append_handler_register_box(Box::new(move |handler| {
                register_precompiles_handles(handler, precompiles.clone());
            }))
            .build();

        evm.transact_commit()
    }?;

    Ok(result)
}

fn validate_configuration<ChainSpecT: RuntimeSpec, BlockchainErrorT, StateErrorT>(
    hardfork: ChainSpecT::Hardfork,
    transaction: &ChainSpecT::Transaction,
) -> Result<(), TransactionError<ChainSpecT, BlockchainErrorT, StateErrorT>> {
    if transaction.max_fee_per_gas().is_some() && Into::into(hardfork) < SpecId::LONDON {
        return Err(TransactionError::Eip1559Unsupported);
    }

    Ok(())
}
