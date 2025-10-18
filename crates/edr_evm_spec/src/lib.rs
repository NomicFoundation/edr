pub mod config;
mod error;
pub mod handler;
pub mod interpreter;
pub mod result;

use edr_chain_spec::{ChainContextSpec, ChainHardfork, ChainSpec, TransactionValidation};
pub use edr_database_components::DatabaseComponentError;
pub use revm_context::{
    Block as BlockEnvTrait, CfgEnv, Context, Database, Evm, Journal, LocalContext,
};
pub use revm_handler::{ExecuteEvm, PrecompileProvider};
use revm_inspector::NoOpInspector;
pub use revm_inspector::{InspectEvm, Inspector};

pub use self::error::{TransactionError, TransactionErrorForChainSpec};
pub use crate::{interpreter::InterpreterResult, result::ExecutionResultAndState};

/// Helper type for a chain-specific [`Context`].
pub type ContextForChainSpec<ChainSpecT, BlockT, DatabaseT> = Context<
    BlockT,
    <ChainSpecT as ChainSpec>::SignedTransaction,
    CfgEnv<<ChainSpecT as ChainHardfork>::Hardfork>,
    DatabaseT,
    Journal<DatabaseT>,
    <ChainSpecT as ChainContextSpec>::Context,
>;

/// A trait for running a transaction in a chain's associated EVM.
pub trait EvmChainSpec: ChainContextSpec + ChainHardfork + ChainSpec {
    /// Type representing a precompile provider.
    type PrecompileProvider<BlockT: BlockEnvTrait, DatabaseT: Database>: Default
        + PrecompileProvider<ContextForChainSpec<Self, BlockT, DatabaseT>, Output = InterpreterResult>;

    /// Runs a transaction inside the chain's EVM without committing the
    /// changes.
    #[allow(clippy::type_complexity)]
    fn dry_run<
        BlockT: BlockEnvTrait,
        DatabaseT: Database,
        PrecompileProviderT: PrecompileProvider<
            ContextForChainSpec<Self, BlockT, DatabaseT>,
            Output = InterpreterResult,
        >,
    >(
        block: BlockT,
        cfg: CfgEnv<Self::Hardfork>,
        transaction: Self::SignedTransaction,
        database: DatabaseT,
        precompile_provider: PrecompileProviderT,
    ) -> Result<
        ExecutionResultAndState<Self::HaltReason>,
        TransactionError<
            DatabaseT::Error,
            <Self::SignedTransaction as TransactionValidation>::ValidationError,
        >,
    > {
        Self::dry_run_with_inspector(
            block,
            cfg,
            transaction,
            database,
            precompile_provider,
            NoOpInspector,
        )
    }

    /// Runs a transaction inside the chain's EVM without committing the
    /// changes, while an inspector is observing the execution.
    #[allow(clippy::type_complexity)]
    fn dry_run_with_inspector<
        BlockT: BlockEnvTrait,
        DatabaseT: Database,
        InspectorT: Inspector<ContextForChainSpec<Self, BlockT, DatabaseT>>,
        PrecompileProviderT: PrecompileProvider<
            ContextForChainSpec<Self, BlockT, DatabaseT>,
            Output = InterpreterResult,
        >,
    >(
        block: BlockT,
        cfg: CfgEnv<Self::Hardfork>,
        transaction: Self::SignedTransaction,
        database: DatabaseT,
        precompile_provider: PrecompileProviderT,
        inspector: InspectorT,
    ) -> Result<
        ExecutionResultAndState<Self::HaltReason>,
        TransactionError<
            DatabaseT::Error,
            <Self::SignedTransaction as TransactionValidation>::ValidationError,
        >,
    >;
}
