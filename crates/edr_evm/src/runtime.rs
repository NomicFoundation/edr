use std::fmt::Debug;

use edr_eth::{
    l1,
    result::{ExecutionResult, InvalidTransaction, ResultAndState},
    spec::HaltReasonTrait,
    transaction::{ExecutableTransaction as _, TransactionValidation},
    Address, HashMap,
};
use revm::{precompile::PrecompileFn, Evm, JournaledState};

use crate::{
    blockchain::SyncBlockchain,
    config::CfgEnv,
    debug::DebugContext,
    precompile::ContextWithCustomPrecompiles,
    spec::RuntimeSpec,
    state::{DatabaseComponents, EvmState, State, StateCommit, WrapDatabaseRef},
    transaction::TransactionError,
};

/// Asynchronous implementation of the Database super-trait
pub type SyncDatabase<'blockchain, 'state, ChainSpecT, BlockchainErrorT, StateErrorT> =
    DatabaseComponents<
        &'blockchain dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateErrorT>,
        &'state dyn State<Error = StateErrorT>,
    >;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainResult<ChainContextT, HaltReasonT: HaltReasonTrait> {
    /// Status of execution
    pub result: ExecutionResult<HaltReasonT>,
    /// State that got updated
    pub state: EvmState,
    /// Chain context
    pub chain_context: ChainContextT,
}

/// Runs a transaction without committing the state.
// `DebugContext` cannot be simplified further
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub fn dry_run<'blockchain, 'evm, ChainSpecT, DebugDataT, BlockchainErrorT, StateT>(
    blockchain: &'blockchain dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateT::Error>,
    state: StateT,
    cfg: CfgEnv<ChainSpecT::Hardfork>,
    transaction: ChainSpecT::SignedTransaction,
    block: ChainSpecT::BlockEnv,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
    debug_context: Option<DebugContext<'evm, ChainSpecT, BlockchainErrorT, DebugDataT, StateT>>,
) -> Result<
    ResultAndState<ChainSpecT::HaltReason>,
    TransactionError<ChainSpecT, BlockchainErrorT, StateT::Error>,
>
where
    'blockchain: 'evm,
    ChainSpecT: RuntimeSpec<
        SignedTransaction: TransactionValidation<ValidationError: From<InvalidTransaction>>,
    >,
    BlockchainErrorT: Debug + Send,
    StateT: State<Error: Debug + Send>,
{
    let database = WrapDatabaseRef(DatabaseComponents { blockchain, state });
    let context = {
        let context = revm::Context {
            block,
            tx: transaction,
            cfg,
            journaled_state: JournaledState::new(cfg.spec.into(), database),
            chain: ChainSpecT::Context::default(),
            error: Ok(()),
        };

        ContextWithCustomPrecompiles {
            context,
            custom_precompiles: custom_precompiles.clone(),
        }
    };

    let result = if let Some(debug_context) = debug_context {
        let evm = revm::Evm::new(context, ChainSpecT::EvmHandler::default());

        let mut evm = Evm::<ChainSpecT::EvmWiring<_, _>>::builder()
            .append_handler_register(debug_context.register_handles_fn)
            .build();

        evm.exec()
    } else {
        let mut evm = revm::Evm::new(context, ChainSpecT::EvmHandler::default());

        evm.exec()
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
    'state,
    ChainSpecT,
    DebugDataT,
    BlockchainErrorT,
    StateT,
>(
    blockchain: &'blockchain dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateT::Error>,
    state: StateT,
    mut cfg: CfgEnv<ChainSpecT::Hardfork>,
    transaction: ChainSpecT::SignedTransaction,
    block: ChainSpecT::BlockEnv,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
    debug_context: Option<DebugContext<'evm, ChainSpecT, BlockchainErrorT, DebugDataT, StateT>>,
) -> Result<
    ChainResult<ChainSpecT::Context, ChainSpecT::HaltReason>,
    TransactionError<ChainSpecT, BlockchainErrorT, StateT::Error>,
>
where
    'blockchain: 'evm,
    'state: 'evm,
    ChainSpecT: RuntimeSpec<
        SignedTransaction: TransactionValidation<ValidationError: From<InvalidTransaction>>,
    >,
    BlockchainErrorT: Debug + Send,
    StateT: State<Error: Debug + Send>,
{
    cfg.disable_balance_check = true;
    cfg.disable_block_gas_limit = true;
    cfg.disable_nonce_check = true;
    dry_run(
        blockchain,
        state,
        cfg,
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
    mut state: StateT,
    cfg: CfgEnv<ChainSpecT::Hardfork>,
    transaction: ChainSpecT::SignedTransaction,
    block: ChainSpecT::BlockEnv,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
    debug_context: Option<DebugContext<'evm, ChainSpecT, BlockchainErrorT, DebugDataT, StateT>>,
) -> Result<
    ExecutionResult<ChainSpecT::HaltReason>,
    TransactionError<ChainSpecT, BlockchainErrorT, StateT::Error>,
>
where
    'blockchain: 'evm,
    ChainSpecT: RuntimeSpec<
        SignedTransaction: TransactionValidation<ValidationError: From<InvalidTransaction>>,
    >,
    BlockchainErrorT: Debug + Send,
    StateT: State + StateCommit,
    StateT::Error: Debug + Send,
{
    let ResultAndState {
        result,
        state: state_diff,
    } = dry_run(
        blockchain,
        state,
        cfg,
        transaction,
        block,
        custom_precompiles,
        debug_context,
    )?;

    state.commit(state_diff);

    Ok(result)
}
