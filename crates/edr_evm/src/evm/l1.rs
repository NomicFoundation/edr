use core::marker::PhantomData;

use edr_eth::{l1::L1ChainSpec, log::ExecutionLog, spec::ChainSpec};
use revm::{state::EvmState, JournalEntry};
use revm_context::CfgEnv;
use revm_context_interface::{
    BlockGetter, CfgGetter, ErrorGetter, Journal, JournalGetter, PerformantContextAccess,
    TransactionGetter,
};
use revm_handler::{
    EthExecution, EthFrame, EthHandler, EthPostExecution, EthPreExecution, EthPrecompileProvider,
    EthValidation, FrameResult,
};
use revm_handler_interface::{Frame, PrecompileProvider};
use revm_interpreter::{
    interpreter::{EthInstructionProvider, EthInterpreter, InstructionProvider},
    FrameInput, Host, InterpreterResult,
};

use super::EvmSpec;
use crate::{
    state::{Database, DatabaseComponentError},
    transaction::TransactionError,
};

pub struct L1EvmSpec<ContextT> {
    phantom: PhantomData<ContextT>,
}

impl<BlockchainErrorT, ContextT, StateErrorT>
    EvmSpec<BlockchainErrorT, L1ChainSpec, ContextT, StateErrorT> for L1EvmSpec<ContextT>
where
    ContextT: BlockGetter<Block = <L1ChainSpec as ChainSpec>::BlockEnv>
        + CfgGetter<Cfg = CfgEnv<<L1ChainSpec as ChainSpec>::Hardfork>>
        + ErrorGetter<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>
        + Host
        + JournalGetter<
            Database: Database<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>,
            Journal: Journal<Entry = JournalEntry, FinalOutput = (EvmState, Vec<ExecutionLog>)>,
        > + PerformantContextAccess<Error = DatabaseComponentError<BlockchainErrorT, StateErrorT>>
        + TransactionGetter<Transaction = <L1ChainSpec as ChainSpec>::SignedTransaction>,
{
    type ValidationHandler =
        EthValidation<ContextT, TransactionError<BlockchainErrorT, L1ChainSpec, StateErrorT>>;

    type PreExecutionHandler =
        EthPreExecution<ContextT, TransactionError<BlockchainErrorT, L1ChainSpec, StateErrorT>>;

    type ExecutionHandler<
        'context,
        FrameT: Frame<
            Context<'context> = ContextT,
            Error = TransactionError<BlockchainErrorT, L1ChainSpec, StateErrorT>,
            FrameInit = FrameInput,
            FrameResult = FrameResult,
        >,
    > = EthExecution<
        ContextT,
        TransactionError<BlockchainErrorT, L1ChainSpec, StateErrorT>,
        FrameT,
    >;

    type PostExecutionHandler = EthPostExecution<
        ContextT,
        TransactionError<BlockchainErrorT, L1ChainSpec, StateErrorT>,
        <L1ChainSpec as ChainSpec>::HaltReason,
    >;

    type Frame<
        InstructionProviderT: InstructionProvider<Host = ContextT, WIRE = EthInterpreter>,
        PrecompileProviderT: PrecompileProvider<
            Context = ContextT,
            Error = TransactionError<BlockchainErrorT, L1ChainSpec, StateErrorT>,
            Output = InterpreterResult,
        >,
    > = EthFrame<
        ContextT,
        TransactionError<BlockchainErrorT, L1ChainSpec, StateErrorT>,
        EthInterpreter,
        PrecompileProviderT,
        InstructionProviderT,
    >;

    type InstructionProvider = EthInstructionProvider<EthInterpreter, ContextT>;

    type PrecompileProvider = EthPrecompileProvider<
        ContextT,
        TransactionError<BlockchainErrorT, L1ChainSpec, StateErrorT>,
    >;
}
