use edr_eth::{
    l1,
    result::{ExecutionResult, ExecutionResultAndState},
    transaction::TransactionValidation,
};
use revm::{handler::EthHandler, JournaledState};
use revm_handler::FrameResult;
use revm_handler_interface::Frame;
use revm_interpreter::FrameInput;

use crate::{
    blockchain::BlockHash,
    config::CfgEnv,
    debug::{ContextExtension, ExtendedContext},
    evm::EvmSpec,
    spec::{ContextForChainSpec, RuntimeSpec},
    state::{DatabaseComponents, State, StateCommit, WrapDatabaseRef},
    transaction::TransactionError,
};

// /// Asynchronous implementation of the Database super-trait
// pub type SyncDatabase<'blockchain, 'state, ChainSpecT, BlockchainErrorT,
// StateErrorT> =     DatabaseComponents<
//         &'blockchain dyn SyncBlockchain<ChainSpecT, BlockchainErrorT,
// StateErrorT>,         &'state dyn State<Error = StateErrorT>,
//     >;

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
    ExecutionResultAndState<ChainSpecT::HaltReason>,
    TransactionError<BlockchainT::Error, ChainSpecT, StateT::Error>,
>
where
    BlockchainT: BlockHash<Error: Send + std::error::Error>,
    ChainSpecT: RuntimeSpec<
        SignedTransaction: TransactionValidation<ValidationError: From<l1::InvalidTransaction>>,
    >,
    EvmSpecT: EvmSpec<
        BlockchainT::Error,
        ChainSpecT,
        ContextForChainSpec<ChainSpecT, WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>>,
        StateT::Error,
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
        EvmSpecT::ValidationHandler::default(),
        EvmSpecT::PreExecutionHandler::default(),
        EvmSpecT::ExecutionHandler::<
            '_,
            EvmSpecT::Frame<EvmSpecT::InstructionProvider, EvmSpecT::PrecompileProvider>,
        >::default(),
        EvmSpecT::PostExecutionHandler::default(),
    );

    // let handler = EvmSpecT::Handler::default();

    let mut evm =
        revm::Evm::<TransactionError<BlockchainT::Error, ChainSpecT, StateT::Error>, _, _>::new(
            context, handler,
        );
    evm.transact()
}

#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub fn dry_run_with_extension<
    'components,
    'context,
    'extension,
    BlockchainT,
    ChainSpecT,
    EvmSpecT,
    ExtensionT,
    FrameT,
    StateT,
>(
    blockchain: BlockchainT,
    state: StateT,
    cfg: CfgEnv<ChainSpecT::Hardfork>,
    transaction: ChainSpecT::SignedTransaction,
    block: ChainSpecT::BlockEnv,
    extension: &'extension mut ContextExtension<ExtensionT, FrameT>,
) -> Result<
    ExecutionResultAndState<ChainSpecT::HaltReason>,
    TransactionError<BlockchainT::Error, ChainSpecT, StateT::Error>,
>
where
    'components: 'context,
    'extension: 'context,
    BlockchainT: BlockHash<Error: Send + std::error::Error> + 'components,
    ChainSpecT: RuntimeSpec<
        SignedTransaction: TransactionValidation<ValidationError: From<l1::InvalidTransaction>>,
    >,
    EvmSpecT: EvmSpec<
        BlockchainT::Error,
        ChainSpecT,
        ExtendedContext<
            'context,
            ContextForChainSpec<
                ChainSpecT,
                WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
            >,
            ExtensionT,
        >,
        StateT::Error,
    >,
    FrameT: Frame<
        Context<'context> = ExtendedContext<
            'context,
            ContextForChainSpec<
                ChainSpecT,
                WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
            >,
            ExtensionT,
        >,
        Error = TransactionError<BlockchainT::Error, ChainSpecT, StateT::Error>,
        FrameInit = FrameInput,
        FrameResult = FrameResult,
    >,
    StateT: State<Error: Send + std::error::Error> + 'components,
{
    let database = WrapDatabaseRef(DatabaseComponents { blockchain, state });

    let context = extension.extend_context(revm::Context {
        block,
        tx: transaction,
        journaled_state: JournaledState::new(cfg.spec.into(), database),
        cfg,
        chain: ChainSpecT::Context::default(),
        error: Ok(()),
    });
    let handler = EthHandler::new(
        EvmSpecT::ValidationHandler::default(),
        EvmSpecT::PreExecutionHandler::default(),
        EvmSpecT::ExecutionHandler::<'context, FrameT>::default(),
        EvmSpecT::PostExecutionHandler::default(),
    );

    let mut evm = revm::Evm::new(context, handler);
    evm.transact()
}

/// Runs a transaction without committing the state, while disabling balance
/// checks and creating accounts for new addresses.
// `DebugContext` cannot be simplified further
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub fn guaranteed_dry_run<BlockchainT, ChainSpecT, EvmSpecT, StateT>(
    blockchain: BlockchainT,
    state: StateT,
    mut cfg: CfgEnv<ChainSpecT::Hardfork>,
    transaction: ChainSpecT::SignedTransaction,
    block: ChainSpecT::BlockEnv,
) -> Result<
    ExecutionResultAndState<ChainSpecT::HaltReason>,
    TransactionError<BlockchainT::Error, ChainSpecT, StateT::Error>,
>
where
    BlockchainT: BlockHash<Error: Send + std::error::Error>,
    ChainSpecT: RuntimeSpec<
        SignedTransaction: TransactionValidation<ValidationError: From<l1::InvalidTransaction>>,
    >,
    EvmSpecT: EvmSpec<
        BlockchainT::Error,
        ChainSpecT,
        ContextForChainSpec<ChainSpecT, WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>>,
        StateT::Error,
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
pub fn run<BlockchainT, ChainSpecT, EvmSpecT, StateT>(
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
        SignedTransaction: TransactionValidation<ValidationError: From<l1::InvalidTransaction>>,
    >,
    StateT: State<Error: Send + std::error::Error> + StateCommit,
{
    let ExecutionResultAndState {
        result,
        state: state_diff,
    } = dry_run(blockchain, &state, cfg, transaction, block)?;

    state.commit(state_diff);

    Ok(result)
}

// /// Runs a transaction, committing the state in the process.
// #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
// pub fn run_with_extension<BlockchainT, ChainSpecT, ExtensionT, FrameT,
// StateT>(     blockchain: BlockchainT,
//     state: StateT,
//     cfg: CfgEnv<ChainSpecT::Hardfork>,
//     transaction: ChainSpecT::SignedTransaction,
//     block: ChainSpecT::BlockEnv,
//     // TODO: REMOVE
//     // custom_precompiles: &HashMap<Address, PrecompileFn>,
//     extension: ContextExtension<ExtensionT, FrameT>,
// ) -> ResultAndState<
//     Result<
//         ExecutionResult<ChainSpecT::HaltReason>,
//         TransactionError<BlockchainT::Error, ChainSpecT, StateT::Error>,
//     >,
//     StateT,
// >
// where
//     BlockchainT: BlockHash<Error: Send + std::error::Error>,
//     ChainSpecT: RuntimeSpec<
//         SignedTransaction: TransactionValidation<ValidationError:
// From<l1::InvalidTransaction>>,     >,
//     FrameT: for<'context> Frame<
//         Context<'context> = ExtendedContext<
//             ContextForChainSpec<
//                 ChainSpecT,
//                 WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
//             >,
//             ExtensionT,
//         >,
//         Error = TransactionError<BlockchainT::Error, ChainSpecT,
// StateT::Error>,         FrameResult = FrameResult,
//     >,
//     StateT: State<Error: Send + std::error::Error> + StateCommit,
// {
//     let ResultAndState { result, mut state } =
//         dry_run_with_extension(blockchain, state, cfg, transaction, block,
// extension);

//     let result = result.map(
//         |ExecutionResultAndState {
//              result,
//              state: changes,
//          }| {
//             state.commit(changes);

//             result
//         },
//     );

//     ResultAndState { result, state }
// }
