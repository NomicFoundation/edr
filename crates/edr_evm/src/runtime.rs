use std::fmt::Debug;

use edr_eth::transaction::{self, SignedTransaction};
use revm::{
    db::{DatabaseComponents, StateRef},
    handler::{CfgEnvWithChainSpec, EnvWithChainSpec},
    primitives::{BlockEnv, ExecutionResult, ResultAndState, SpecId},
    DatabaseCommit, Evm,
};

use crate::{
    blockchain::SyncBlockchain,
    chain_spec::{ChainSpec, L1ChainSpec},
    debug::DebugContext,
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
#[allow(clippy::type_complexity)]
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub fn dry_run<'blockchain, 'evm, 'overrides, 'state, DebugDataT, BlockchainErrorT, StateErrorT>(
    blockchain: &'blockchain dyn SyncBlockchain<L1ChainSpec, BlockchainErrorT, StateErrorT>,
    state: &'state dyn SyncState<StateErrorT>,
    state_overrides: &'overrides StateOverrides,
    cfg: CfgEnvWithChainSpec<L1ChainSpec>,
    transaction: transaction::Signed,
    block: BlockEnv,
    debug_context: Option<
        DebugContext<
            'evm,
            L1ChainSpec,
            BlockchainErrorT,
            DebugDataT,
            StateRefOverrider<'overrides, &'evm dyn SyncState<StateErrorT>>,
        >,
    >,
) -> Result<ResultAndState<L1ChainSpec>, TransactionError<L1ChainSpec, BlockchainErrorT, StateErrorT>>
where
    'blockchain: 'evm,
    'state: 'evm,
    BlockchainErrorT: Debug + Send,
    StateErrorT: Debug + Send,
{
    validate_configuration::<L1ChainSpec, BlockchainErrorT, StateErrorT>(
        cfg.spec_id,
        &transaction,
    )?;

    let state_overrider = StateRefOverrider::new(state_overrides, state);

    let env = EnvWithChainSpec::new_with_cfg_env(cfg, block, transaction);
    let result = {
        let evm_builder = Evm::builder().with_ref_db(DatabaseComponents {
            state: state_overrider,
            block_hash: blockchain,
        });

        if let Some(debug_context) = debug_context {
            let mut evm = evm_builder
                .with_chain_spec::<L1ChainSpec>()
                .with_external_context(debug_context.data)
                .with_env_with_handler_cfg(env)
                .append_handler_register(debug_context.register_handles_fn)
                .build();

            evm.transact()
        } else {
            let mut evm = evm_builder
                .with_chain_spec::<L1ChainSpec>()
                .with_env_with_handler_cfg(env)
                .build();
            evm.transact()
        }
    };

    result.map_err(TransactionError::from)
}

/// Runs a transaction without committing the state, while disabling balance
/// checks and creating accounts for new addresses.
// `DebugContext` cannot be simplified further
#[allow(clippy::type_complexity)]
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub fn guaranteed_dry_run<
    'blockchain,
    'evm,
    'overrides,
    'state,
    DebugDataT,
    BlockchainErrorT,
    StateErrorT,
>(
    blockchain: &'blockchain dyn SyncBlockchain<L1ChainSpec, BlockchainErrorT, StateErrorT>,
    state: &'state dyn SyncState<StateErrorT>,
    state_overrides: &'overrides StateOverrides,
    mut cfg: CfgEnvWithChainSpec<L1ChainSpec>,
    mut transaction: transaction::Signed,
    block: BlockEnv,
    debug_context: Option<
        DebugContext<
            'evm,
            L1ChainSpec,
            BlockchainErrorT,
            DebugDataT,
            StateRefOverrider<'overrides, &'evm dyn SyncState<StateErrorT>>,
        >,
    >,
) -> Result<ResultAndState<L1ChainSpec>, TransactionError<L1ChainSpec, BlockchainErrorT, StateErrorT>>
where
    'blockchain: 'evm,
    'state: 'evm,
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
        debug_context,
    )
}

/// Runs a transaction, committing the state in the process.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub fn run<'blockchain, 'evm, BlockchainErrorT, DebugDataT, StateT>(
    blockchain: &'blockchain dyn SyncBlockchain<L1ChainSpec, BlockchainErrorT, StateT::Error>,
    state: StateT,
    cfg: CfgEnvWithChainSpec<L1ChainSpec>,
    transaction: transaction::Signed,
    block: BlockEnv,
    debug_context: Option<DebugContext<'evm, L1ChainSpec, BlockchainErrorT, DebugDataT, StateT>>,
) -> Result<
    ExecutionResult<L1ChainSpec>,
    TransactionError<L1ChainSpec, BlockchainErrorT, StateT::Error>,
>
where
    'blockchain: 'evm,
    BlockchainErrorT: Debug + Send,
    StateT: StateRef + DatabaseCommit,
    StateT::Error: Debug + Send,
{
    validate_configuration::<L1ChainSpec, BlockchainErrorT, StateT::Error>(
        cfg.spec_id,
        &transaction,
    )?;

    let env = EnvWithChainSpec::new_with_cfg_env(cfg, block, transaction);
    let evm_builder = Evm::builder().with_ref_db(DatabaseComponents {
        state,
        block_hash: blockchain,
    });

    let result = if let Some(debug_context) = debug_context {
        let mut evm = evm_builder
            .with_chain_spec::<L1ChainSpec>()
            .with_external_context(debug_context.data)
            .with_env_with_handler_cfg(env)
            .append_handler_register(debug_context.register_handles_fn)
            .build();

        evm.transact_commit()
    } else {
        let mut evm = evm_builder
            .with_chain_spec::<L1ChainSpec>()
            .with_env_with_handler_cfg(env)
            .build();

        evm.transact_commit()
    }?;

    Ok(result)
}

fn validate_configuration<ChainSpecT: ChainSpec, BlockchainErrorT, StateErrorT>(
    hardfork: ChainSpecT::Hardfork,
    transaction: &transaction::Signed,
) -> Result<(), TransactionError<L1ChainSpec, BlockchainErrorT, StateErrorT>> {
    if transaction.max_fee_per_gas().is_some() && Into::into(hardfork) < SpecId::LONDON {
        return Err(TransactionError::Eip1559Unsupported);
    }

    Ok(())
}
