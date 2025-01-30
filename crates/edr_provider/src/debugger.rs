mod context;

use edr_eth::spec::{ChainSpec, HaltReasonTrait};
use edr_evm::{
    evm::EvmSpec,
    instruction::InspectableInstructionProvider,
    interpreter::EthInterpreter,
    spec::RuntimeSpec,
    trace::{TraceCollector, TraceCollectorFrame},
};

pub use self::context::{DebuggerContext, DebuggerContextWithPrecompiles};
use crate::{
    console_log::{ConsoleLogCollector, ConsoleLogCollectorFrame},
    mock::{Mocker, MockingFrame},
};

pub struct Debugger<HaltReasonT: HaltReasonTrait> {
    pub console_logger: ConsoleLogCollector,
    pub mocker: Mocker,
    pub trace_collector: TraceCollector<HaltReasonT>,
}

impl<HaltReasonT: HaltReasonTrait> Debugger<HaltReasonT> {
    /// Creates a new instance with the provided mocker.
    /// If verbose is true, full stack and memory will be recorded for each
    /// step.
    pub fn with_mocker(mocker: Mocker, verbose: bool) -> Self {
        Self {
            console_logger: ConsoleLogCollector::default(),
            mocker,
            trace_collector: TraceCollector::new(verbose),
        }
    }
}

/// Helper type for a frame that combines all features of the
/// [`DebuggerContext`] for the provided precompile provider type.
pub type DebuggerFrameWithPrecompileProvider<BlockchainErrorT, ChainSpecT, ContextT, PrecompileProviderT, StateErrorT> =
    ConsoleLogCollectorFrame<
        MockingFrame<
            TraceCollectorFrame<
                <<ChainSpecT as RuntimeSpec>::Evm<BlockchainErrorT, ContextT, StateErrorT> as EvmSpec<BlockchainErrorT, ChainSpecT, ContextT, StateErrorT>>::Frame<
                    InspectableInstructionProvider<
                        ContextT,
                        EthInterpreter,
                        <<ChainSpecT as RuntimeSpec>::Evm<BlockchainErrorT, ContextT, StateErrorT> as EvmSpec<BlockchainErrorT, ChainSpecT, ContextT, StateErrorT>>::InstructionProvider,
                    >,
                    PrecompileProviderT,
                >,
                <ChainSpecT as ChainSpec>::HaltReason,
            >
        >
    >;

/// Helper type for a frame that combines all features of the
/// [`DebuggerContext`].
pub type DebuggerFrame<BlockchainErrorT, ChainSpecT, ContextT, StateErrorT> =
    DebuggerFrameWithPrecompileProvider<
        BlockchainErrorT,
        ChainSpecT,
        ContextT,
        <<ChainSpecT as RuntimeSpec>::Evm<BlockchainErrorT, ContextT, StateErrorT> as EvmSpec<
            BlockchainErrorT,
            ChainSpecT,
            ContextT,
            StateErrorT,
        >>::PrecompileProvider,
        StateErrorT,
    >;
