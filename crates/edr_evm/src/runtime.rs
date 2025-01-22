use edr_eth::{
    result::{ExecutionResult, InvalidTransaction, ResultAndState},
    transaction::TransactionValidation,
};
use revm::{handler::EthHandler, Database, JournaledState};
use revm_handler::FrameResult;
use revm_handler_interface::Frame;

use crate::{
    blockchain::{BlockHash, SyncBlockchain},
    config::CfgEnv,
    debug::{ContextExtension, ExtendedContext},
    spec::{ContextForChainSpec, FrameForChainSpec, RuntimeSpec},
    state::{DatabaseComponentError, DatabaseComponents, State, StateCommit, WrapDatabaseRef},
    transaction::TransactionError,
};

/// Asynchronous implementation of the Database super-trait
pub type SyncDatabase<'blockchain, 'state, ChainSpecT, BlockchainErrorT, StateErrorT> =
    DatabaseComponents<
        &'blockchain dyn SyncBlockchain<ChainSpecT, BlockchainErrorT, StateErrorT>,
        &'state dyn State<Error = StateErrorT>,
    >;

/// Runs a transaction without committing the state.
// `DebugContext` cannot be simplified further
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub fn dry_run<BlockchainT, ChainSpecT, StateT>(
    blockchain: BlockchainT,
    state: StateT,
    cfg: CfgEnv<ChainSpecT::Hardfork>,
    transaction: ChainSpecT::SignedTransaction,
    block: ChainSpecT::BlockEnv,
) -> Result<
    ResultAndState<ChainSpecT::HaltReason>,
    TransactionError<BlockchainT::Error, ChainSpecT, StateT::Error>,
>
where
    BlockchainT: BlockHash<Error: Send + std::error::Error>,
    ChainSpecT: RuntimeSpec<
        SignedTransaction: TransactionValidation<ValidationError: From<InvalidTransaction>>,
    >,
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

    let context = revm::Context {
        block,
        tx: transaction,
        journaled_state: JournaledState::new(cfg.spec.into(), database),
        cfg,
        chain: ChainSpecT::Context::default(),
        error: Ok(()),
    };
    let handler = EthHandler::new(
        ChainSpecT::EvmValidationHandler::<BlockchainT::Error, _, StateT::Error>::default(),
        ChainSpecT::EvmPreExecutionHandler::<BlockchainT::Error, _, StateT::Error>::default(),
        ChainSpecT::EvmExecutionHandler::<
            BlockchainT::Error,
            _,
            FrameForChainSpec<BlockchainT::Error, ChainSpecT, _, StateT::Error>,
            StateT::Error,
        >::default(),
        ChainSpecT::EvmPostExecutionHandler::<BlockchainT::Error, _, StateT::Error>::default(),
    );

    let mut evm = revm::Evm::new(context, handler);
    evm.transact()
}

#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub fn dry_run_with_extension<
    'database,
    BlockchainErrorT,
    ChainSpecT,
    DatabaseT,
    ExtensionT,
    FrameT,
    StateErrorT,
>(
    database: DatabaseT,
    cfg: CfgEnv<ChainSpecT::Hardfork>,
    transaction: ChainSpecT::SignedTransaction,
    block: ChainSpecT::BlockEnv,
    extension: ContextExtension<ExtensionT, FrameT>,
) -> Result<
    ResultAndState<ChainSpecT::HaltReason>,
    TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>,
>
where
    DatabaseT: Database<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>> + 'database,
    // BlockchainT: BlockHash<Error: Send + std::error::Error> + 'database,
    ChainSpecT: RuntimeSpec<
        SignedTransaction: TransactionValidation<ValidationError: From<InvalidTransaction>>,
    >,
    FrameT: Frame<
        Context = ExtendedContext<ContextForChainSpec<ChainSpecT, DatabaseT>, ExtensionT>,
        Error = TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>,
        FrameResult = FrameResult,
    >,
    // StateT: State<Error: Send + std::error::Error> + 'database,
{
    let context = extension.extend_context(revm::Context {
        block,
        tx: transaction,
        journaled_state: JournaledState::new(cfg.spec.into(), database),
        cfg,
        chain: ChainSpecT::Context::default(),
        error: Ok(()),
    });
    let handler = EthHandler::new(
        ChainSpecT::EvmValidationHandler::<BlockchainErrorT, _, StateErrorT>::default(),
        ChainSpecT::EvmPreExecutionHandler::<BlockchainErrorT, _, StateErrorT>::default(),
        ChainSpecT::EvmExecutionHandler::<BlockchainErrorT, _, FrameT, StateErrorT>::default(),
        ChainSpecT::EvmPostExecutionHandler::<BlockchainErrorT, _, StateErrorT>::default(),
    );

    let mut evm = revm::Evm::new(context, handler);
    evm.transact()
}

/// Runs a transaction without committing the state, while disabling balance
/// checks and creating accounts for new addresses.
// `DebugContext` cannot be simplified further
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub fn guaranteed_dry_run<BlockchainT, ChainSpecT, StateT>(
    blockchain: BlockchainT,
    state: StateT,
    mut cfg: CfgEnv<ChainSpecT::Hardfork>,
    transaction: ChainSpecT::SignedTransaction,
    block: ChainSpecT::BlockEnv,
) -> Result<
    ResultAndState<ChainSpecT::HaltReason>,
    TransactionError<BlockchainT::Error, ChainSpecT, StateT::Error>,
>
where
    BlockchainT: BlockHash<Error: Send + std::error::Error>,
    ChainSpecT: RuntimeSpec<
        SignedTransaction: TransactionValidation<ValidationError: From<InvalidTransaction>>,
    >,
    StateT: State<Error: Send + std::error::Error>,
{
    cfg.disable_balance_check = true;
    cfg.disable_block_gas_limit = true;
    cfg.disable_nonce_check = true;
    dry_run(blockchain, state, cfg, transaction, block)
}

// #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
// pub fn guaranteed_dry_run_with_extension<
//     'database,
//     BlockchainT,
//     ChainSpecT,
//     ExtensionT,
//     FrameT,
//     StateT,
// >(
//     blockchain: BlockchainT,
//     state: StateT,
//     mut cfg: CfgEnv<ChainSpecT::Hardfork>,
//     transaction: ChainSpecT::SignedTransaction,
//     block: ChainSpecT::BlockEnv,
//     extension: ContextExtension<ExtensionT, FrameT>,
// ) -> Result<
//     ResultAndState<ChainSpecT::HaltReason>,
//     TransactionError<BlockchainT::Error, ChainSpecT, StateT::Error>,
// >
// where
//     BlockchainT: BlockHash<Error: Send + std::error::Error> + 'database,
//     ChainSpecT: RuntimeSpec<
//         SignedTransaction: TransactionValidation<ValidationError:
// From<InvalidTransaction>>,     >,
//     FrameT: for<'context> Frame<
//         Context<'context> = ExtendedContext<
//             ContextForChainSpec<
//                 ChainSpecT,
//                 Box<
//                     dyn Database<Error =
// DatabaseComponentError<BlockchainT::Error, StateT::Error>>
//                         + 'database,
//                 >,
//             >,
//             ExtensionT,
//         >,
//         Error = TransactionError<ChainSpecT, BlockchainT::Error,
// StateT::Error>,         FrameResult = FrameResult,
//     >,
//     StateT: State<Error: Send + std::error::Error> + 'database,
// {
//     cfg.disable_balance_check = true;
//     cfg.disable_block_gas_limit = true;
//     cfg.disable_nonce_check = true;
//     dry_run_with_extension(blockchain, state, cfg, transaction, block,
// extension) }

/// Runs a transaction, committing the state in the process.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub fn run<BlockchainT, ChainSpecT, StateT>(
    blockchain: BlockchainT,
    mut state: StateT,
    cfg: CfgEnv<ChainSpecT::Hardfork>,
    transaction: ChainSpecT::SignedTransaction,
    block: ChainSpecT::BlockEnv,
    // TODO: REMOVE
    // custom_precompiles: &HashMap<Address, PrecompileFn>,
) -> Result<
    ExecutionResult<ChainSpecT::HaltReason>,
    TransactionError<BlockchainT::Error, ChainSpecT, StateT::Error>,
>
where
    BlockchainT: BlockHash<Error: Send + std::error::Error>,
    ChainSpecT: RuntimeSpec<
        SignedTransaction: TransactionValidation<ValidationError: From<InvalidTransaction>>,
    >,
    StateT: Clone + State<Error: Send + std::error::Error> + StateCommit,
{
    let ResultAndState {
        result,
        state: state_diff,
    } = dry_run(blockchain, state.clone(), cfg, transaction, block)?;

    state.commit(state_diff);

    Ok(result)
}

// /// Runs a transaction, committing the state in the process.
// #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
// pub fn run_with_extension<'database, BlockchainT, ChainSpecT, ExtensionT,
// FrameT, StateT>(     blockchain: BlockchainT,
//     mut state: StateT,
//     cfg: CfgEnv<ChainSpecT::Hardfork>,
//     transaction: ChainSpecT::SignedTransaction,
//     block: ChainSpecT::BlockEnv,
//     // TODO: REMOVE
//     // custom_precompiles: &HashMap<Address, PrecompileFn>,
//     extension: ContextExtension<ExtensionT, FrameT>,
// ) -> Result<
//     ExecutionResult<ChainSpecT::HaltReason>,
//     TransactionError<BlockchainT::Error, ChainSpecT, StateT::Error>,
// >
// where
//     BlockchainT: BlockHash<Error: Send + std::error::Error> + 'database,
//     ChainSpecT: RuntimeSpec<
//         SignedTransaction: TransactionValidation<ValidationError:
// From<InvalidTransaction>>,     >,
//     FrameT: for<'context> Frame<
//         Context<'context> = ExtendedContext<
//             ContextForChainSpec<
//                 ChainSpecT,
//                 Box<
//                     dyn Database<Error =
// DatabaseComponentError<BlockchainT::Error, StateT::Error>>
//                         + 'database,
//                 >,
//             >,
//             ExtensionT,
//         >,
//         Error = TransactionError<ChainSpecT, BlockchainT::Error,
// StateT::Error>,         FrameResult = FrameResult,
//     >,
//     StateT: Clone + State<Error: Send + std::error::Error> + StateCommit +
// 'database, {
//     let ResultAndState {
//         result,
//         state: state_diff,
//     } = dry_run_with_extension(
//         blockchain,
//         state.clone(),
//         cfg,
//         transaction,
//         block,
//         extension,
//     )?;

//     state.commit(state_diff);

//     Ok(result)
// }
