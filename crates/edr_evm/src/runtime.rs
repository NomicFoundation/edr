use std::fmt::Debug;

use edr_eth::{
    db::{DatabaseComponents, StateRef},
    result::{ExecutionResult, InvalidTransaction, ResultAndState},
    transaction::{SignedTransaction as _, TransactionValidation},
    Address, HashMap, Precompile, SpecId,
};
use revm::{
    handler::{CfgEnvWithEvmWiring, EnvWithEvmWiring},
    ContextPrecompile, DatabaseCommit, Evm,
};

use crate::{
    blockchain::SyncBlockchain,
    chain_spec::ChainSpec,
    debug::DebugContext,
    precompiles::register_precompiles_handles,
    state::{StateOverrides, StateRefOverrider, SyncState},
    transaction::TransactionError,
};

/// Asynchronous implementation of the Database super-trait
pub type SyncDatabase<'blockchain, 'state, ChainSpecT, BlockchainErrorT, StateErrorT> =
    DatabaseComponents<
        &'state dyn StateRef<Error = StateErrorT>,
        &'blockchain dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateErrorT>,
    >;

/// Runs a transaction without committing the state.
// `DebugContext` cannot be simplified further
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub fn dry_run<
    'blockchain,
    'evm,
    'overrides,
    'state,
    ChainSpecT,
    DebugDataT,
    BlockchainErrorT,
    StateErrorT,
>(
    blockchain: &'blockchain dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateErrorT>,
    state: &'state dyn SyncState<StateErrorT>,
    state_overrides: &'overrides StateOverrides,
    cfg: CfgEnvWithEvmWiring<ChainSpecT>,
    transaction: ChainSpecT::Transaction,
    block: ChainSpecT::Block,
    custom_precompiles: &HashMap<Address, Precompile>,
    debug_context: Option<
        DebugContext<
            'evm,
            ChainSpecT,
            BlockchainErrorT,
            DebugDataT,
            StateRefOverrider<'overrides, &'evm dyn SyncState<StateErrorT>>,
        >,
    >,
) -> Result<ResultAndState<ChainSpecT>, TransactionError<ChainSpecT, BlockchainErrorT, StateErrorT>>
where
    'blockchain: 'evm,
    'state: 'evm,
    ChainSpecT: ChainSpec<
        Block: Default,
        Transaction: Default + TransactionValidation<ValidationError: From<InvalidTransaction>>,
    >,
    BlockchainErrorT: Debug + Send,
    StateErrorT: Debug + Send,
{
    validate_configuration::<ChainSpecT, BlockchainErrorT, StateErrorT>(cfg.spec_id, &transaction)?;

    let state_overrider = StateRefOverrider::new(state_overrides, state);

    let env = EnvWithEvmWiring::new_with_cfg_env(cfg, block, transaction);
    let result = {
        let evm_builder = Evm::builder().with_ref_db(DatabaseComponents {
            state: state_overrider,
            block_hash: blockchain,
        });

        let precompiles: HashMap<Address, ContextPrecompile<ChainSpecT, _>> = custom_precompiles
            .iter()
            .map(|(address, precompile)| (*address, ContextPrecompile::from(precompile.clone())))
            .collect();

        if let Some(debug_context) = debug_context {
            let mut evm = evm_builder
                .with_chain_spec::<ChainSpecT>()
                .with_external_context(debug_context.data)
                .with_env_with_handler_cfg(env)
                .append_handler_register(debug_context.register_handles_fn)
                .append_handler_register_box(Box::new(move |handler| {
                    register_precompiles_handles(handler, precompiles.clone());
                }))
                .build();

            evm.transact()
        } else {
            let mut evm = evm_builder
                .with_chain_spec::<ChainSpecT>()
                .with_env_with_handler_cfg(env)
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
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub fn guaranteed_dry_run<
    'blockchain,
    'evm,
    'overrides,
    'state,
    ChainSpecT,
    DebugDataT,
    BlockchainErrorT,
    StateErrorT,
>(
    blockchain: &'blockchain dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateErrorT>,
    state: &'state dyn SyncState<StateErrorT>,
    state_overrides: &'overrides StateOverrides,
    mut cfg: CfgEnvWithEvmWiring<ChainSpecT>,
    transaction: ChainSpecT::Transaction,
    block: ChainSpecT::Block,
    custom_precompiles: &HashMap<Address, Precompile>,
    debug_context: Option<
        DebugContext<
            'evm,
            ChainSpecT,
            BlockchainErrorT,
            DebugDataT,
            StateRefOverrider<'overrides, &'evm dyn SyncState<StateErrorT>>,
        >,
    >,
) -> Result<ResultAndState<ChainSpecT>, TransactionError<ChainSpecT, BlockchainErrorT, StateErrorT>>
where
    'blockchain: 'evm,
    'state: 'evm,
    ChainSpecT: ChainSpec<
        Block: Default,
        Transaction: Default + TransactionValidation<ValidationError: From<InvalidTransaction>>,
    >,
    BlockchainErrorT: Debug + Send,
    StateErrorT: Debug + Send,
{
    cfg.disable_balance_check = true;
    cfg.disable_block_gas_limit = true;
    cfg.disable_nonce_check = true;
    dry_run(
        blockchain,
        state,
        state_overrides,
        cfg,
        transaction,
        block,
        custom_precompiles,
        debug_context,
    )
}

/// Runs a transaction, committing the state in the process.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub fn run<'blockchain, 'evm, ChainSpecT, BlockchainErrorT, DebugDataT, StateT>(
    blockchain: &'blockchain dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateT::Error>,
    state: StateT,
    cfg: CfgEnvWithEvmWiring<ChainSpecT>,
    transaction: ChainSpecT::Transaction,
    block: ChainSpecT::Block,
    custom_precompiles: &HashMap<Address, Precompile>,
    debug_context: Option<DebugContext<'evm, ChainSpecT, BlockchainErrorT, DebugDataT, StateT>>,
) -> Result<
    ExecutionResult<ChainSpecT>,
    TransactionError<ChainSpecT, BlockchainErrorT, StateT::Error>,
>
where
    'blockchain: 'evm,
    ChainSpecT: ChainSpec<
        Block: Default,
        Transaction: Default + TransactionValidation<ValidationError: From<InvalidTransaction>>,
    >,
    BlockchainErrorT: Debug + Send,
    StateT: StateRef + DatabaseCommit,
    StateT::Error: Debug + Send,
{
    validate_configuration::<ChainSpecT, BlockchainErrorT, StateT::Error>(
        cfg.spec_id,
        &transaction,
    )?;

    let env = EnvWithEvmWiring::new_with_cfg_env(cfg, block, transaction);
    let evm_builder = Evm::builder().with_ref_db(DatabaseComponents {
        state,
        block_hash: blockchain,
    });

    let precompiles: HashMap<Address, ContextPrecompile<ChainSpecT, _>> = custom_precompiles
        .iter()
        .map(|(address, precompile)| (*address, ContextPrecompile::from(precompile.clone())))
        .collect();

    let result = if let Some(debug_context) = debug_context {
        let mut evm = evm_builder
            .with_chain_spec::<ChainSpecT>()
            .with_external_context(debug_context.data)
            .with_env_with_handler_cfg(env)
            .append_handler_register(debug_context.register_handles_fn)
            .append_handler_register_box(Box::new(move |handler| {
                register_precompiles_handles(handler, precompiles.clone());
            }))
            .build();

        evm.transact_commit()
    } else {
        let mut evm = evm_builder
            .with_chain_spec::<ChainSpecT>()
            .with_env_with_handler_cfg(env)
            .append_handler_register_box(Box::new(move |handler| {
                register_precompiles_handles(handler, precompiles.clone());
            }))
            .build();

        evm.transact_commit()
    }?;

    Ok(result)
}

fn validate_configuration<ChainSpecT: ChainSpec, BlockchainErrorT, StateErrorT>(
    hardfork: ChainSpecT::Hardfork,
    transaction: &ChainSpecT::Transaction,
) -> Result<(), TransactionError<ChainSpecT, BlockchainErrorT, StateErrorT>> {
    if transaction.max_fee_per_gas().is_some() && Into::into(hardfork) < SpecId::LONDON {
        return Err(TransactionError::Eip1559Unsupported);
    }

    Ok(())
}
