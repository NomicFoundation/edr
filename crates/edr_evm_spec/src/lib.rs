pub mod handler;
pub mod interpreter;
pub mod result;

use edr_chain_spec::{ChainHardfork, ChainSpec};
pub use edr_database_components::DatabaseComponentError;
use edr_state_api::EvmState;
pub use revm_context::{CfgEnv, Context, Database, Evm, Journal};
use revm_handler::ExecuteEvm;
pub use revm_handler::PrecompileProvider;
pub use revm_inspector::{InspectEvm, Inspector};

use crate::{
    interpreter::InterpreterResult,
    result::{EVMErrorForChain, ExecutionResult},
};

/// Helper type for a chain-specific [`Context`].
pub type ContextForChainSpec<ChainSpecT, DatabaseT> = Context<
    <ChainSpecT as ChainSpec>::BlockEnv,
    <ChainSpecT as ChainSpec>::SignedTransaction,
    CfgEnv<<ChainSpecT as ChainHardfork>::Hardfork>,
    DatabaseT,
    Journal<DatabaseT>,
    <ChainSpecT as ChainSpec>::Context,
>;

/// A trait for defining a chain's associated EVM specification.
pub trait ChainEvmSpec: ChainHardfork + ChainSpec {
    /// Type representing an EVM specification for the provided context and
    /// error types.
    type Evm<
        BlockchainErrorT,
        DatabaseT: Database<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        InspectorT: Inspector<ContextForChainSpec<Self, DatabaseT>>,
        PrecompileProviderT: PrecompileProvider<ContextForChainSpec<Self, DatabaseT>, Output = InterpreterResult>,
        StateErrorT,
    >: ExecuteEvm<
        ExecutionResult = ExecutionResult<Self::HaltReason>,
        State = EvmState,
        Error = EVMErrorForChain<Self, BlockchainErrorT, StateErrorT>,
        Tx = Self::SignedTransaction,
    > + InspectEvm<Inspector = InspectorT>;

    /// Type representing a precompile provider.
    type PrecompileProvider<
        BlockchainErrorT,
        DatabaseT: Database<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        StateErrorT,
    >: Default + PrecompileProvider<ContextForChainSpec<Self, DatabaseT>, Output = InterpreterResult>;

    /// Constructs an EVM instance with the provided context and inspector.
    #[allow(clippy::type_complexity)]
    fn evm_with_inspector<
        BlockchainErrorT,
        DatabaseT: Database<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        InspectorT: Inspector<ContextForChainSpec<Self, DatabaseT>>,
        PrecompileProviderT: PrecompileProvider<ContextForChainSpec<Self, DatabaseT>, Output = InterpreterResult>,
        StateErrorT,
    >(
        block: Self::BlockEnv,
        cfg: CfgEnv<Self::Hardfork>,
        transaction: Self::SignedTransaction,
        database: DatabaseT,
        inspector: InspectorT,
        precompile_provider: PrecompileProviderT,
    ) -> Result<
        Self::Evm<BlockchainErrorT, DatabaseT, InspectorT, PrecompileProviderT, StateErrorT>,
        DatabaseT::Error,
    >;
}
