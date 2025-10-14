mod error;
pub mod handler;
pub mod interpreter;
pub mod result;

use edr_chain_spec::{ChainContextSpec, ChainHardfork, ChainSpec, TransactionValidation};
pub use edr_database_components::DatabaseComponentError;
pub use revm_context::{CfgEnv, Context, Database, Evm, Journal};
pub use revm_handler::{ExecuteEvm, PrecompileProvider};
use revm_inspector::NoOpInspector;
pub use revm_inspector::{InspectEvm, Inspector};

pub use self::error::{TransactionError, TransactionErrorForChainSpec};
pub use crate::{interpreter::InterpreterResult, result::ExecutionResultAndState};

/// Helper type for a chain-specific [`Context`].
pub type ContextForChainSpec<ChainSpecT, DatabaseT> = Context<
    <ChainSpecT as ChainSpec>::BlockEnv,
    <ChainSpecT as ChainSpec>::SignedTransaction,
    CfgEnv<<ChainSpecT as ChainHardfork>::Hardfork>,
    DatabaseT,
    Journal<DatabaseT>,
    <ChainSpecT as ChainContextSpec>::Context,
>;

/// A trait for running a transaction in a chain's associated EVM.
pub trait ChainEvmSpec: ChainContextSpec + ChainHardfork + ChainSpec {
    /// Type representing a precompile provider.
    type PrecompileProvider<DatabaseT: Database>: Default
        + PrecompileProvider<ContextForChainSpec<Self, DatabaseT>, Output = InterpreterResult>;

    /// Runs a transaction inside the chain's EVM without committing the
    /// changes.
    #[allow(clippy::type_complexity)]
    fn dry_run<
        DatabaseT: Database,
        PrecompileProviderT: PrecompileProvider<ContextForChainSpec<Self, DatabaseT>, Output = InterpreterResult>,
    >(
        block: Self::BlockEnv,
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
        DatabaseT: Database,
        InspectorT: Inspector<ContextForChainSpec<Self, DatabaseT>>,
        PrecompileProviderT: PrecompileProvider<ContextForChainSpec<Self, DatabaseT>, Output = InterpreterResult>,
    >(
        block: Self::BlockEnv,
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
