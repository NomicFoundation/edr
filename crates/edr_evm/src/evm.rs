pub mod l1;

use edr_eth::{log::ExecutionLog, result::ExecutionResultAndState, spec::ChainSpec};
pub use revm::{
    handler,
    interpreter,
    // wiring::{evm_wiring::EvmWiring as PrimitiveEvmWiring, result},
    Context,
    // ContextPrecompile, EvmContext, EvmWiring, FrameOrResult, FrameResult, InnerEvmContext,
    JournalEntry,
};
use revm::{state::EvmState, JournaledState};
use revm_context_interface::{
    BlockGetter, CfgGetter, DatabaseGetter, ErrorGetter, Journal, JournalGetter,
    PerformantContextAccess, TransactionGetter,
};
use revm_handler::FrameResult;
use revm_handler_interface::{
    ExecutionHandler, Frame, PostExecutionHandler, PreExecutionHandler, PrecompileProvider,
    ValidationHandler,
};
use revm_interpreter::{
    interpreter::{EthInterpreter, InstructionProvider},
    FrameInput, Host, InterpreterResult,
};

use crate::{
    blockchain::BlockHash,
    config::CfgEnv,
    result::EVMErrorForChain,
    state::{Database, DatabaseComponentError, DatabaseComponents, State},
    transaction::TransactionError,
};

pub type EvmForChainSpec<BlockchainT, ChainSpecT, StateT> = revm::Evm<
    EVMErrorForChain<ChainSpecT, <BlockchainT as BlockHash>::Error, <StateT as State>::Error>,
    EvmContextForChainSpec<BlockchainT, ChainSpecT, StateT>,
    // TODO: Custom handler
>;

pub type EvmContextForChainSpec<BlockchainT, ChainSpecT, StateT> = revm::Context<
    <ChainSpecT as ChainSpec>::BlockEnv,
    <ChainSpecT as ChainSpec>::SignedTransaction,
    CfgEnv,
    DatabaseComponents<BlockchainT, StateT>,
    JournaledState<DatabaseComponents<BlockchainT, StateT>>,
    <ChainSpecT as ChainSpec>::Context,
>;

pub trait EvmSpec<BlockchainErrorT, ChainSpecT, ContextT, StateErrorT>
where
    ChainSpecT: ChainSpec,
    ContextT: TransactionGetter
        + BlockGetter
        + JournalGetter
        + CfgGetter
        + DatabaseGetter<
            Database: Database<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
        > + ErrorGetter<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>
        + JournalGetter<
            Journal: Journal<
                FinalOutput = (EvmState, Vec<ExecutionLog>),
                Database = <ContextT as DatabaseGetter>::Database,
            >,
        > + Host
        + PerformantContextAccess<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
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
        FrameT: Frame<
            Context<'context> = ContextT,
            Error = TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>,
            FrameInit = FrameInput,
            FrameResult = FrameResult
        >,
    >: Default + ExecutionHandler<
        Context = ContextT,
        Error = TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>,
        ExecResult = FrameResult,
        Frame = FrameT
    >;

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
    >: for<'context> Frame<
        Context<'context> = ContextT,
        Error = TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>,
        FrameInit = FrameInput,
        FrameResult = FrameResult,
    >;

    /// Type representing an EVM instruction provider.
    type InstructionProvider: InstructionProvider<Host = ContextT, WIRE = EthInterpreter>;

    /// Type representing an EVM precompile provider.
    type PrecompileProvider: PrecompileProvider<
        Context = ContextT,
        Error = TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>,
        Output = InterpreterResult,
    >;
}
