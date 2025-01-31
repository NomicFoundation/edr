use std::marker::PhantomData;

use edr_eth::spec::{ChainSpec, HaltReasonTrait};
use revm::{context_interface::JournalGetter, handler::FrameResult, interpreter::FrameInput};
use revm_handler_interface::{Frame, FrameOrResultGen};
use revm_interpreter::interpreter::EthInterpreter;

use super::context::Eip3155TracerMutGetter;
use crate::{
    evm::EvmSpec, instruction::InspectableInstructionProvider, spec::RuntimeSpec,
    trace::RawTracerFrame,
};

/// A frame that wraps the inner frame and notifies the tracer every time a
/// frame ends.
pub struct Eip3155TracerFrame<
    FrameT: Frame<FrameInit = FrameInput, FrameResult = FrameResult>,
    HaltReasonT: HaltReasonTrait,
> {
    inner: FrameT,
    _phantom: PhantomData<HaltReasonT>,
}

impl<
        FrameT: Frame<FrameInit = FrameInput, FrameResult = FrameResult>,
        HaltReasonT: HaltReasonTrait,
    > Eip3155TracerFrame<FrameT, HaltReasonT>
{
    /// Creates a new instance.
    fn new(inner: FrameT) -> Self {
        Self {
            inner,
            _phantom: PhantomData,
        }
    }

    /// Notifies the tracer that a frame has ended.
    fn notify_frame_end<ContextT: Eip3155TracerMutGetter>(
        context: &mut ContextT,
        result: &FrameResult,
    ) {
        let eip3155_tracer = context.eip3155_tracer_mut();
        match result {
            FrameResult::Call(outcome) => {
                eip3155_tracer.on_inner_frame_result(&outcome.result);
            }
            FrameResult::Create(outcome) => {
                eip3155_tracer.on_inner_frame_result(&outcome.result);
            }
            // TODO: https://github.com/NomicFoundation/edr/issues/427
            FrameResult::EOFCreate(_outcome) => unreachable!("EDR doesn't support EOF yet."),
        }
    }
}

impl<ContextT, FrameT, HaltReasonT> Frame for Eip3155TracerFrame<FrameT, HaltReasonT>
where
    ContextT: Eip3155TracerMutGetter + JournalGetter,
    FrameT: for<'context> Frame<
        Context<'context> = ContextT,
        FrameInit = FrameInput,
        FrameResult = FrameResult,
    >,
    HaltReasonT: HaltReasonTrait,
{
    type Context<'context> = ContextT;

    type FrameInit = FrameT::FrameInit;

    type FrameResult = FrameT::FrameResult;

    type Error = FrameT::Error;

    fn init_first(
        context: &mut Self::Context<'_>,
        frame_input: Self::FrameInit,
    ) -> Result<FrameOrResultGen<Self, Self::FrameResult>, Self::Error> {
        FrameT::init_first(context, frame_input).map(|frame| frame.map_frame(Self::new))
    }

    fn final_return(
        context: &mut Self::Context<'_>,
        result: &mut Self::FrameResult,
    ) -> Result<(), Self::Error> {
        FrameT::final_return(context, result)
    }

    fn init(
        &self,
        context: &mut Self::Context<'_>,
        frame_input: Self::FrameInit,
    ) -> Result<FrameOrResultGen<Self, Self::FrameResult>, Self::Error> {
        self.inner
            .init(context, frame_input)
            .map(|frame| frame.map_frame(Self::new))
    }

    fn run(
        &mut self,
        context: &mut Self::Context<'_>,
    ) -> Result<FrameOrResultGen<Self::FrameInit, Self::FrameResult>, Self::Error> {
        self.inner.run(context)
    }

    fn return_result(
        &mut self,
        context: &mut Self::Context<'_>,
        result: Self::FrameResult,
    ) -> Result<(), Self::Error> {
        Self::notify_frame_end(context, &result);

        self.inner.return_result(context, result)
    }
}

/// Helper type for a frame that combines EIP-3155 and raw tracers for the
/// provided precompile provider type.
pub type Eip3155AndRawTracersFrameWithPrecompileProvider<BlockchainErrorT, ChainSpecT, ContextT, PrecompileProviderT, StateErrorT> =
    Eip3155TracerFrame<
        RawTracerFrame<
            <<ChainSpecT as RuntimeSpec>::Evm<BlockchainErrorT, ContextT, StateErrorT> as EvmSpec<BlockchainErrorT, ChainSpecT, ContextT, StateErrorT>>::Frame<
                InspectableInstructionProvider<
                    ContextT,
                    EthInterpreter,
                    <<ChainSpecT as RuntimeSpec>::Evm<BlockchainErrorT, ContextT, StateErrorT> as EvmSpec<BlockchainErrorT, ChainSpecT, ContextT, StateErrorT>>::InstructionProvider,
                >,
                PrecompileProviderT,
            >,
            <ChainSpecT as ChainSpec>::HaltReason,
        >,
        <ChainSpecT as ChainSpec>::HaltReason,
    >;

/// Helper type for a frame that combines EIP-3155 and raw tracers.
pub type Eip3155AndRawTracersFrame<BlockchainErrorT, ChainSpecT, ContextT, StateErrorT> =
    Eip3155AndRawTracersFrameWithPrecompileProvider<
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
