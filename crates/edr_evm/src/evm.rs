/// EVM specification for L1 chains.
pub mod l1;

use edr_eth::{log::ExecutionLog, result::ExecutionResultAndState, spec::ChainSpec};
use revm::{state::EvmState, JournaledState};
pub use revm::{Context, JournalEntry};
use revm_context_interface::{
    BlockGetter, CfgGetter, DatabaseGetter, ErrorGetter, Journal, JournalGetter,
    PerformantContextAccess, TransactionGetter,
};
use revm_database_interface::WrapDatabaseRef;
pub use revm_handler::FrameResult;
pub use revm_handler_interface::{
    ExecutionHandler, Frame, FrameOrResultGen, PostExecutionHandler, PreExecutionHandler,
    PrecompileProvider, ValidationHandler,
};
use revm_interpreter::{
    interpreter::{EthInterpreter, InstructionProvider},
    FrameInput, Host, InterpreterResult,
};

use crate::{
    blockchain::BlockHash,
    config::CfgEnv,
    extension::ExtendedContext,
    instruction::InspectableInstruction,
    result::EVMErrorForChain,
    spec::{ContextForChainSpec, RuntimeSpec},
    state::{Database, DatabaseComponentError, DatabaseComponents, State},
    transaction::TransactionError,
};

/// Helper type for a chain-specific [`Evm`] with a default [`revm::Context`].
pub type EvmForChainSpec<'context, BlockchainT, ChainSpecT, StateT> = revm::Evm<
    'context,
    EVMErrorForChain<ChainSpecT, <BlockchainT as BlockHash>::Error, <StateT as State>::Error>,
    EvmContextForChainSpec<BlockchainT, ChainSpecT, StateT>,
    // TODO: Custom handler
>;

/// Helper type for a chain-specific [`EvmSpec`] with a default
/// [`revm::Context`].
pub type EvmSpecForDefaultContext<BlockchainT, ChainSpecT, StateT> =
    <ChainSpecT as RuntimeSpec>::Evm<
        <BlockchainT as BlockHash>::Error,
        ContextForChainSpec<ChainSpecT, WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>>,
        <StateT as State>::Error,
    >;

/// Helper type for a chain-specific [`EvmSpec`] with an [`ExtendedContext`].
pub type EvmSpecForExtendedContext<'context, BlockchainT, ChainSpecT, ExtensionT, StateT> =
    <ChainSpecT as RuntimeSpec>::Evm<
        <BlockchainT as BlockHash>::Error,
        ExtendedContext<
            'context,
            ContextForChainSpec<
                ChainSpecT,
                WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
            >,
            ExtensionT,
        >,
        <StateT as State>::Error,
    >;

/// Helper type for a chain-specific [`revm::Context`].
pub type EvmContextForChainSpec<BlockchainT, ChainSpecT, StateT> = revm::Context<
    <ChainSpecT as ChainSpec>::BlockEnv,
    <ChainSpecT as ChainSpec>::SignedTransaction,
    CfgEnv,
    DatabaseComponents<BlockchainT, StateT>,
    JournaledState<DatabaseComponents<BlockchainT, StateT>>,
    <ChainSpecT as ChainSpec>::Context,
>;

/// Trait for a chain-specific EVM specification with the provided context.
pub trait EvmSpec<BlockchainErrorT, ChainSpecT, ContextT, StateErrorT>
where
    ChainSpecT: ChainSpec,
    ContextT: BlockGetter
        + CfgGetter
        + DatabaseGetter<
            Database: Database<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        > + ErrorGetter<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>
        + Host
        + JournalGetter<
            Journal: Journal<
                FinalOutput = (EvmState, Vec<ExecutionLog>),
                Database = <ContextT as DatabaseGetter>::Database,
            >,
        > + PerformantContextAccess<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>
        + TransactionGetter,
{
    /// Type representing an EVM validation handler.
    type ValidationHandler: Default
        + ValidationHandler<
            Context = ContextT,
            Error = TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>,
        >;

    /// Type representing an EVM pre-execution handler.
    type PreExecutionHandler: Default
        + PreExecutionHandler<
            Context = ContextT,
            Error = TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>,
        >;

    /// Type representing an EVM execution handler.
    type ExecutionHandler<
        'context,
        FrameT: 'context + Frame<
            Context<'context> = ContextT,
            Error = TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>,
            FrameInit = FrameInput,
            FrameResult = FrameResult
        >,
    >: Default + ExecutionHandler<
        'context,
        Context = ContextT,
        Error = TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>,
        ExecResult = FrameResult,
        Frame = FrameT
    >
    where
        BlockchainErrorT: 'context,
        ContextT: 'context,
        StateErrorT: 'context;

    /// Type representing an EVM post-execution handler.
    type PostExecutionHandler: Default
        + PostExecutionHandler<
            Context = ContextT,
            Error = TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>,
            ExecResult = FrameResult,
            Output = ExecutionResultAndState<ChainSpecT::HaltReason>,
        >;

    /// Type representing an EVM frame.
    type Frame<
        InstructionProviderT: InstructionProvider<Host = ContextT, WIRE = EthInterpreter>,
        PrecompileProviderT: PrecompileProvider<Context = ContextT, Error = TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>, Output = InterpreterResult>
    >: for<'context99> Frame<
        Context<'context99> = ContextT,
        Error = TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>,
        FrameInit = FrameInput,
        FrameResult = FrameResult,
    >;

    /// Type representing an EVM instruction provider.
    type InstructionProvider: InstructionProvider<
        Host = ContextT,
        Instruction: Clone + Into<InspectableInstruction<ContextT, EthInterpreter>>,
        WIRE = EthInterpreter,
    >;

    /// Type representing an EVM precompile provider.
    type PrecompileProvider: PrecompileProvider<
        Context = ContextT,
        Error = TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>,
        Output = InterpreterResult,
    >;
}
