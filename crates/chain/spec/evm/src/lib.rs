pub mod config;
mod error;
pub mod handler;
pub mod interpreter;
pub mod result;

use edr_chain_spec::{
    ChainSpec, ContextChainSpec, EvmHardforkChainSpec, EvmTransactionValidationError,
    ProtocolHardforkChainSpec, TransactionValidation,
};
pub use edr_database_components::DatabaseComponentError;
pub use revm_context::{
    Block as BlockEnvTrait, CfgEnv, Context, ContextError, ContextTr as ContextTrait, Database,
    Evm, Journal, JournalEntry, JournalTr as JournalTrait, LocalContext,
};
pub use revm_handler::{ExecuteEvm, PrecompileProvider};
use revm_inspector::NoOpInspector;
pub use revm_inspector::{InspectEvm, Inspector};

pub use self::error::{TransactionError, TransactionErrorForChainSpec};
pub use crate::{interpreter::InterpreterResult, result::ExecutionResultAndState};

/// Helper type for a chain-specific [`Context`].
pub type ContextForChainSpec<ChainSpecT, BlockEnvT, DatabaseT> = Context<
    BlockEnvT,
    <ChainSpecT as ChainSpec>::SignedTransaction,
    CfgEnv<<ChainSpecT as EvmHardforkChainSpec>::EvmHardfork>,
    DatabaseT,
    Journal<DatabaseT>,
    <ChainSpecT as ContextChainSpec>::Context,
>;

/// Retypes a [`CfgEnv`] keyed on the chain's protocol-level hardfork into one
/// keyed on its EVM-level hardfork, preserving all other fields.
pub fn to_evm_cfg_env<ChainSpecT: ProtocolHardforkChainSpec>(
    cfg: CfgEnv<ChainSpecT::ProtocolHardfork>,
) -> CfgEnv<ChainSpecT::EvmHardfork> {
    let spec: ChainSpecT::EvmHardfork = cfg.spec.into();
    // Pass the original gas params through to avoid recomputing them.
    let gas_params = cfg.gas_params.clone();
    cfg.with_spec_and_gas_params(spec, gas_params)
}

/// Trait for specifying the types for running a transaction in a chain's
/// associated EVM.
pub trait EvmChainSpec:
    ChainSpec<
        SignedTransaction: TransactionValidation<
            ValidationError: From<EvmTransactionValidationError>,
        >,
    > + ContextChainSpec
    + ProtocolHardforkChainSpec
{
    /// Type representing a precompile provider.
    type PrecompileProvider<BlockT: BlockEnvTrait, DatabaseT: Database>: PrecompileProvider<
        ContextForChainSpec<Self, BlockT, DatabaseT>,
        Output = InterpreterResult,
    >;

    /// Constructs the precompile provider for the given hardfork.
    fn new_precompile_provider<BlockT: BlockEnvTrait, DatabaseT: Database>(
        hardfork: Self::ProtocolHardfork,
    ) -> Self::PrecompileProvider<BlockT, DatabaseT>;

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
        cfg: CfgEnv<Self::ProtocolHardfork>,
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
        cfg: CfgEnv<Self::ProtocolHardfork>,
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
