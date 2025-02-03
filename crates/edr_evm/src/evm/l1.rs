use core::marker::PhantomData;

use edr_eth::log::ExecutionLog;
use revm::{state::EvmState, JournalEntry};
use revm_context::CfgEnv;
use revm_context_interface::{
    BlockGetter, CfgGetter, ErrorGetter, Journal, JournalGetter, PerformantContextAccess,
    TransactionGetter,
};
use revm_handler::{
    EthExecution, EthFrame, EthPostExecution, EthPreExecution, EthPrecompileProvider,
    EthValidation, FrameResult,
};
use revm_handler_interface::{Frame, PrecompileProvider};
use revm_interpreter::{
    interpreter::{EthInstructionProvider, EthInterpreter, InstructionProvider},
    FrameInput, Host, InterpreterResult,
};

use super::EvmSpec;
use crate::{
    spec::RuntimeSpec,
    state::{Database, DatabaseComponentError},
    transaction::TransactionError,
};

/// An EVM specification for L1 chains, given the provided context.
pub struct L1EvmSpec<ChainSpecT: RuntimeSpec, ContextT> {
    phantom: PhantomData<(ChainSpecT, ContextT)>,
}

impl<BlockchainErrorT, ChainSpecT: RuntimeSpec, ContextT, StateErrorT>
    EvmSpec<BlockchainErrorT, ChainSpecT, ContextT, StateErrorT> for L1EvmSpec<ChainSpecT, ContextT>
where
    // TODO: Remove once TransactionError no longer uses ChainSpecT as generic
    ChainSpecT: 'static,
    ContextT: BlockGetter<Block = ChainSpecT::BlockEnv>
        + CfgGetter<Cfg = CfgEnv<ChainSpecT::Hardfork>>
        + ErrorGetter<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>
        + Host
        + JournalGetter<
            Database: Database<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
            Journal: Journal<Entry = JournalEntry, FinalOutput = (EvmState, Vec<ExecutionLog>)>,
        > + PerformantContextAccess<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>
        + TransactionGetter<Transaction = ChainSpecT::SignedTransaction>,
{
    type ValidationHandler =
        EthValidation<ContextT, TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>>;

    type PreExecutionHandler =
        EthPreExecution<ContextT, TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>>;

    type ExecutionHandler<
        'context,
        FrameT: 'context
            + Frame<
                Context<'context> = ContextT,
                Error = TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>,
                FrameInit = FrameInput,
                FrameResult = FrameResult,
            >,
    >
        = EthExecution<
        'context,
        ContextT,
        TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>,
        FrameT,
    >
    where
        BlockchainErrorT: 'context,
        ContextT: 'context,
        StateErrorT: 'context;

    type PostExecutionHandler = EthPostExecution<
        ContextT,
        TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>,
        ChainSpecT::HaltReason,
    >;

    type Frame<
        InstructionProviderT: InstructionProvider<Host = ContextT, WIRE = EthInterpreter>,
        PrecompileProviderT: PrecompileProvider<
            Context = ContextT,
            Error = TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>,
            Output = InterpreterResult,
        >,
    > = EthFrame<
        ContextT,
        TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>,
        EthInterpreter,
        PrecompileProviderT,
        InstructionProviderT,
    >;

    type InstructionProvider = EthInstructionProvider<EthInterpreter, ContextT>;

    type PrecompileProvider = EthPrecompileProvider<
        ContextT,
        TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>,
    >;
}
