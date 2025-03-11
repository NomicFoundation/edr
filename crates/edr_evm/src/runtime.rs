use edr_eth::{
    l1,
    result::{ExecutionResult, ExecutionResultAndState},
    transaction::TransactionValidation,
};
use revm::{ExecuteEvm, InspectEvm, Inspector, Journal};

use crate::{
    blockchain::BlockHash,
    config::CfgEnv,
    result::EVMError,
    spec::{ContextForChainSpec, RuntimeSpec},
    state::{DatabaseComponents, State, StateCommit, WrapDatabaseRef},
    transaction::{TransactionError, TransactionErrorForChainSpec},
};

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
    TransactionErrorForChainSpec<BlockchainT::Error, ChainSpecT, StateT::Error>,
>
where
    BlockchainT: BlockHash<Error: Send + std::error::Error>,
    ChainSpecT: RuntimeSpec<
        SignedTransaction: TransactionValidation<ValidationError: From<l1::InvalidTransaction>>,
    >,
    StateT: State<Error: Send + std::error::Error>,
{
    let database = WrapDatabaseRef(DatabaseComponents { blockchain, state });

    let context = revm::Context {
        block,
        tx: transaction,
        journaled_state: Journal::new(cfg.spec.into(), database),
        cfg,
        chain: ChainSpecT::Context::default(),
        error: Ok(()),
    };

    let mut evm = ChainSpecT::evm(context);
    evm.replay().map_err(|error| match error {
        EVMError::Transaction(error) => ChainSpecT::cast_transaction_error(error),
        EVMError::Header(error) => TransactionError::InvalidHeader(error),
        EVMError::Database(error) => error.into(),
        EVMError::Custom(error) => TransactionError::Custom(error),
        EVMError::Precompile(error) => TransactionError::Precompile(error),
    })
}

/// Runs a transaction while observing with an inspector, without committing the
/// state.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub fn dry_run_with_inspector<BlockchainT, ChainSpecT, InspectorT, StateT>(
    blockchain: BlockchainT,
    state: StateT,
    cfg: CfgEnv<ChainSpecT::Hardfork>,
    transaction: ChainSpecT::SignedTransaction,
    block: ChainSpecT::BlockEnv,
    inspector: &mut InspectorT,
) -> Result<
    ExecutionResultAndState<ChainSpecT::HaltReason>,
    TransactionErrorForChainSpec<BlockchainT::Error, ChainSpecT, StateT::Error>,
>
where
    BlockchainT: BlockHash<Error: Send + std::error::Error>,
    ChainSpecT: RuntimeSpec<
        SignedTransaction: TransactionValidation<ValidationError: From<l1::InvalidTransaction>>,
    >,
    StateT: State<Error: Send + std::error::Error>,
    InspectorT: Inspector<
        ContextForChainSpec<ChainSpecT, WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>>,
    >,
{
    let database = WrapDatabaseRef(DatabaseComponents { blockchain, state });

    let context = revm::Context {
        block,
        tx: transaction,
        journaled_state: Journal::new(cfg.spec.into(), database),
        cfg,
        chain: ChainSpecT::Context::default(),
        error: Ok(()),
    };

    let mut evm = ChainSpecT::evm_with_inspector(context, inspector);
    evm.inspect_replay().map_err(|error| match error {
        EVMError::Transaction(error) => ChainSpecT::cast_transaction_error(error),
        EVMError::Header(error) => TransactionError::InvalidHeader(error),
        EVMError::Database(error) => error.into(),
        EVMError::Custom(error) => TransactionError::Custom(error),
        EVMError::Precompile(error) => TransactionError::Precompile(error),
    })
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
    TransactionErrorForChainSpec<BlockchainT::Error, ChainSpecT, StateT::Error>,
>
where
    BlockchainT: BlockHash<Error: Send + std::error::Error>,
    ChainSpecT: RuntimeSpec<
        SignedTransaction: TransactionValidation<ValidationError: From<l1::InvalidTransaction>>,
    >,
    StateT: State<Error: Send + std::error::Error>,
{
    set_guarantees(&mut cfg);

    dry_run::<_, ChainSpecT, _>(blockchain, state, cfg, transaction, block)
}

/// Runs a transaction while observing with an inspector, without committing the
/// state, while disabling balance checks and creating accounts for new
/// addresses.
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
pub fn guaranteed_dry_run_with_extension<BlockchainT, ChainSpecT, InspectorT, StateT>(
    blockchain: BlockchainT,
    state: StateT,
    mut cfg: CfgEnv<ChainSpecT::Hardfork>,
    transaction: ChainSpecT::SignedTransaction,
    block: ChainSpecT::BlockEnv,
    extension: &mut InspectorT,
) -> Result<
    ExecutionResultAndState<ChainSpecT::HaltReason>,
    TransactionErrorForChainSpec<BlockchainT::Error, ChainSpecT, StateT::Error>,
>
where
    BlockchainT: BlockHash<Error: Send + std::error::Error>,
    ChainSpecT: RuntimeSpec<
        SignedTransaction: TransactionValidation<ValidationError: From<l1::InvalidTransaction>>,
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
        extension,
    )
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
    TransactionErrorForChainSpec<BlockchainT::Error, ChainSpecT, StateT::Error>,
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
    } = dry_run::<_, ChainSpecT, _>(blockchain, &state, cfg, transaction, block)?;

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
