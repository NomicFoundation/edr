use edr_eth::{
    l1,
    log::ExecutionLog,
    result::{ExecutionResult, InvalidTransaction, ResultAndState},
    spec::HaltReasonTrait,
    transaction::{ExecutableTransaction as _, TransactionValidation},
    Address, HashMap,
};
use edr_rpc_eth::receipt::Block;
use revm::{handler::EthHandler, precompile::PrecompileFn, Evm, JournaledState};
use revm_context_interface::{
    block::BlockSetter, CfgGetter, DatabaseGetter, ErrorGetter, Journal, JournalGetter,
    PerformantContextAccess, TransactionGetter,
};
use revm_interpreter::Host;

use crate::{
    blockchain::{BlockHash, SyncBlockchain},
    config::CfgEnv,
    debug::EvmExtension,
    precompile::ContextWithCustomPrecompiles,
    spec::{ContextForChainSpec, FrameForChainSpec, RuntimeSpec},
    state::{
        DatabaseComponentError, DatabaseComponents, EvmState, State, StateCommit, WrapDatabaseRef,
    },
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
pub fn dry_run<BlockchainT, ChainSpecT, ConstructorT, OuterContextT, StateT>(
    blockchain: BlockchainT,
    state: StateT,
    cfg: CfgEnv<ChainSpecT::Hardfork>,
    transaction: ChainSpecT::SignedTransaction,
    block: ChainSpecT::BlockEnv,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
    extension: Option<
        &EvmExtension<
            ConstructorT,
            ContextForChainSpec<BlockchainT, ChainSpecT, StateT>,
            OuterContextT,
        >,
    >,
) -> Result<
    ResultAndState<ChainSpecT::HaltReason>,
    TransactionError<ChainSpecT, BlockchainT::Error, StateT::Error>,
>
where
    BlockchainT: BlockHash<Error: Send + std::error::Error>,
    ChainSpecT: RuntimeSpec<
        SignedTransaction: TransactionValidation<ValidationError: From<InvalidTransaction>>,
    >,
    ConstructorT: Fn(ContextForChainSpec<BlockchainT, ChainSpecT, StateT>) -> OuterContextT,
    OuterContextT: BlockSetter
        + CfgGetter
        + DatabaseGetter
        + ErrorGetter<Error = DatabaseComponentError<BlockchainT::Error, StateT::Error>>
        + Host
        + JournalGetter<
            Database = WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
            Journal: Journal<
                Database = WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
                FinalOutput = (EvmState, Vec<ExecutionLog>),
            >,
        > + PerformantContextAccess<Error = DatabaseComponentError<BlockchainT::Error, StateT::Error>>
        + TransactionGetter,
    StateT: State<Error: Send + std::error::Error>,
{
    let database = WrapDatabaseRef(DatabaseComponents { blockchain, state });
    // let context = {
    //     let context = revm::Context {
    //         block,
    //         tx: transaction,
    //         cfg,
    //         journaled_state: JournaledState::new(cfg.spec.into(), database),
    //         chain: ChainSpecT::Context::default(),
    //         error: Ok(()),
    //     };

    //     ContextWithCustomPrecompiles {
    //         context,
    //         custom_precompiles: custom_precompiles.clone(),
    //     }
    // };

    let result = if let Some(extension) = extension {
        let context = (extension.context_constructor)(revm::Context {
            block,
            tx: transaction,
            cfg,
            journaled_state: JournaledState::new(cfg.spec.into(), database),
            chain: ChainSpecT::Context::default(),
            error: Ok(()),
        });
        let handler = EthHandler::new(
            ChainSpecT::EvmValidationHandler::<
                OuterContextT,
                ChainSpecT::EvmError<BlockchainT::Error, StateT::Error>,
            >::default(),
            ChainSpecT::EvmPreExecutionHandler::<
                OuterContextT,
                ChainSpecT::EvmError<BlockchainT::Error, StateT::Error>,
            >::default(),
            ChainSpecT::EvmExecutionHandler::<
                OuterContextT,
                ChainSpecT::EvmError<BlockchainT::Error, StateT::Error>,
                FrameForChainSpec<BlockchainT::Error, ChainSpecT, OuterContextT, StateT::Error>,
            >::default(),
            ChainSpecT::EvmPostExecutionHandler::<
                OuterContextT,
                ChainSpecT::EvmError<BlockchainT::Error, StateT::Error>,
            >::default(),
        );

        let evm = revm::Evm::new(context, handler);
        evm.transact()
    } else {
        let context = revm::Context {
            block,
            tx: transaction,
            cfg,
            journaled_state: JournaledState::new(cfg.spec.into(), database),
            chain: ChainSpecT::Context::default(),
            error: Ok(()),
        };
        let handler = EthHandler::new(
            ChainSpecT::EvmValidationHandler::<
                _,
                ChainSpecT::EvmError<BlockchainT::Error, StateT::Error>,
            >::default(),
            ChainSpecT::EvmPreExecutionHandler::<
                _,
                ChainSpecT::EvmError<BlockchainT::Error, StateT::Error>,
            >::default(),
            ChainSpecT::EvmExecutionHandler::<
                _,
                ChainSpecT::EvmError<BlockchainT::Error, StateT::Error>,
                FrameForChainSpec<BlockchainT::Error, ChainSpecT, _, StateT::Error>,
            >::default(),
            ChainSpecT::EvmPostExecutionHandler::<
                _,
                ChainSpecT::EvmError<BlockchainT::Error, StateT::Error>,
            >::default(),
        );

        let mut evm = revm::Evm::new(context, handler);

        evm.transact()
    };

    result.map_err(TransactionError::from)
}

/// Runs a transaction without committing the state, while disabling balance
/// checks and creating accounts for new addresses.
// `DebugContext` cannot be simplified further
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub fn guaranteed_dry_run<BlockchainErrorT, ChainSpecT, ConstructorT, OuterContextT, StateT>(
    blockchain: &dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateT::Error>,
    state: StateT,
    mut cfg: CfgEnv<ChainSpecT::Hardfork>,
    transaction: ChainSpecT::SignedTransaction,
    block: ChainSpecT::BlockEnv,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
    extension: Option<
        &EvmExtension<
            ConstructorT,
            ContextForChainSpec<
                &dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateT::Error>,
                ChainSpecT,
                StateT,
            >,
            OuterContextT,
        >,
    >,
) -> Result<
    ChainResult<ChainSpecT::Context, ChainSpecT::HaltReason>,
    TransactionError<ChainSpecT, BlockchainErrorT, StateT::Error>,
>
where
    BlockchainErrorT: Send + std::error::Error,
    ChainSpecT: RuntimeSpec<
        SignedTransaction: TransactionValidation<ValidationError: From<InvalidTransaction>>,
    >,
    ConstructorT: Fn(
        ContextForChainSpec<
            &dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateT::Error>,
            ChainSpecT,
            StateT,
        >,
    ) -> OuterContextT,
    StateT: State<Error: Send + std::error::Error>,
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
        extension,
    )
}

/// Runs a transaction, committing the state in the process.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
#[allow(clippy::too_many_arguments)]
pub fn run<BlockchainErrorT, ChainSpecT, ConstructorT, OuterContextT, StateT>(
    blockchain: &dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateT::Error>,
    mut state: StateT,
    cfg: CfgEnv<ChainSpecT::Hardfork>,
    transaction: ChainSpecT::SignedTransaction,
    block: ChainSpecT::BlockEnv,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
    extension: Option<
        &EvmExtension<
            ConstructorT,
            ContextForChainSpec<
                &dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateT::Error>,
                ChainSpecT,
                StateT,
            >,
            OuterContextT,
        >,
    >,
) -> Result<
    ExecutionResult<ChainSpecT::HaltReason>,
    TransactionError<ChainSpecT, BlockchainErrorT, StateT::Error>,
>
where
    BlockchainErrorT: Send + std::error::Error,
    ChainSpecT: RuntimeSpec<
        SignedTransaction: TransactionValidation<ValidationError: From<InvalidTransaction>>,
    >,
    ConstructorT: Fn(
        ContextForChainSpec<
            &dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateT::Error>,
            ChainSpecT,
            StateT,
        >,
    ) -> OuterContextT,
    StateT: State<Error: Send + std::error::Error> + StateCommit,
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
        extension,
    )?;

    state.commit(state_diff);

    Ok(result)
}
