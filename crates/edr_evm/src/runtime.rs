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
    evm::{EvmSpec, EvmSpecForDefaultContext, EvmSpecForExtendedContext},
    extension::{ContextExtension, ExtendedContext},
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
    StateT: State<Error: Send + std::error::Error>,
{
    let database = WrapDatabaseRef(DatabaseComponents { blockchain, state });

    // let context = {
    //     let context = revm::Context {
    //         block,
    //         tx: transaction,
    //         journaled_state: JournaledState::new(cfg.spec.into(), database),
    //         cfg,
    //         chain: ChainSpecT::Context::default(),
    //         error: Ok(()),
    //     };

    //     ContextWithCustomPrecompiles {
    //         context,
    //         custom_precompiles,
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
        <EvmSpecForDefaultContext::<BlockchainT, ChainSpecT, StateT> as EvmSpec<_, _, _, _>>::ValidationHandler::default(),
        <EvmSpecForDefaultContext::<BlockchainT, ChainSpecT, StateT> as EvmSpec<_, _, _, _>>::PreExecutionHandler::default(),
        <EvmSpecForDefaultContext::<BlockchainT, ChainSpecT, StateT> as EvmSpec<_, _, _, _>>::ExecutionHandler::<'_,
            <ChainSpecT::Evm<
                BlockchainT::Error,
                ContextForChainSpec<
                    ChainSpecT,
                    WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
                >,
                StateT::Error,
            > as EvmSpec<_, _, _, _>>::Frame<
                <EvmSpecForDefaultContext::<BlockchainT, ChainSpecT, StateT> as EvmSpec<_, _, _, _>>::InstructionProvider,
                <EvmSpecForDefaultContext::<BlockchainT, ChainSpecT, StateT> as EvmSpec<_, _, _, _>>::PrecompileProvider,
            >,
        >::default(),
        <EvmSpecForDefaultContext::<BlockchainT, ChainSpecT, StateT> as EvmSpec<_, _, _, _>>::PostExecutionHandler::default(),
    );

    let mut evm =
        revm::Evm::<TransactionError<BlockchainT::Error, ChainSpecT, StateT::Error>, _, _>::new(
            context, handler,
        );
    evm.transact()
}

/// Runs a transaction using an extension without committing the state.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub fn dry_run_with_extension<
    'context,
    'extension,
    BlockchainT,
    ChainSpecT,
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
    'extension: 'context,
    BlockchainT: BlockHash<Error: Send + std::error::Error> + 'context,
    ChainSpecT: 'context
        + RuntimeSpec<
            SignedTransaction: TransactionValidation<ValidationError: From<l1::InvalidTransaction>>,
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
    StateT: State<Error: Send + std::error::Error> + 'context,
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
        <EvmSpecForExtendedContext<'context, BlockchainT, ChainSpecT, ExtensionT, StateT> as EvmSpec<
            _,
            _,
            _,
            _,
        >>::ValidationHandler::default(),
        <EvmSpecForExtendedContext<'context, BlockchainT, ChainSpecT, ExtensionT, StateT> as EvmSpec<
            _,
            _,
            _,
            _,
        >>::PreExecutionHandler::default(),
        <EvmSpecForExtendedContext<'context, BlockchainT, ChainSpecT, ExtensionT, StateT> as EvmSpec<
            _,
            _,
            _,
            _,
        >>::ExecutionHandler::<'context, FrameT>::default(),
        <EvmSpecForExtendedContext<'context, BlockchainT, ChainSpecT, ExtensionT, StateT> as EvmSpec<
            _,
            _,
            _,
            _,
        >>::PostExecutionHandler::default(),
    );

    let mut evm = revm::Evm::new(context, handler);
    evm.transact()
}

/// Runs a transaction without committing the state, while disabling balance
/// checks and creating accounts for new addresses.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub fn guaranteed_dry_run<BlockchainT, ChainSpecT, StateT>(
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
    StateT: State<Error: Send + std::error::Error>,
{
    set_guarantees(&mut cfg);

    dry_run(blockchain, state, cfg, transaction, block)
}

/// Runs a transaction using an extension without committing the state, while
/// disabling balance checks and creating accounts for new addresses.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub fn guaranteed_dry_run_with_extension<
    'components,
    'context,
    'extension,
    BlockchainT,
    ChainSpecT,
    ExtensionT,
    FrameT,
    StateT,
>(
    blockchain: BlockchainT,
    state: StateT,
    mut cfg: CfgEnv<ChainSpecT::Hardfork>,
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
    ChainSpecT: 'context
        + RuntimeSpec<
            SignedTransaction: TransactionValidation<ValidationError: From<l1::InvalidTransaction>>,
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
    set_guarantees(&mut cfg);

    dry_run_with_extension(blockchain, state, cfg, transaction, block, extension)
}

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

fn set_guarantees<HardforkT: Into<l1::SpecId>>(config: &mut CfgEnv<HardforkT>) {
    config.disable_balance_check = true;
    config.disable_block_gas_limit = true;
    config.disable_nonce_check = true;
}
