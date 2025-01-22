use std::marker::PhantomData;

use edr_eth::spec::{ChainSpec, HaltReasonTrait};
use revm::{context_interface::JournalGetter, handler::FrameResult, interpreter::FrameInput};
use revm_handler_interface::{Frame, FrameOrResultGen};
use revm_interpreter::interpreter::EthInterpreter;

use super::context::Eip3155TracerGetter;
use crate::{
    blockchain::BlockHash,
    instruction::InspectableInstructionProvider,
    spec::{ContextForChainSpec, RuntimeSpec},
    state::{DatabaseComponents, State, WrapDatabaseRef},
    trace::TraceCollectorFrame,
};

pub type Eip3155TracerFrameForChainSpec<BlockchainT, ChainSpecT, StateT> = Eip3155TracerFrame<
    <ChainSpecT as RuntimeSpec>::EvmFrame<
        <BlockchainT as BlockHash>::Error,
        ContextForChainSpec<ChainSpecT, WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>>,
        <ChainSpecT as RuntimeSpec>::EvmInstructionProvider<
            ContextForChainSpec<
                ChainSpecT,
                WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
            >,
        >,
        <ChainSpecT as RuntimeSpec>::EvmPrecompileProvider<
            <BlockchainT as BlockHash>::Error,
            ContextForChainSpec<
                ChainSpecT,
                WrapDatabaseRef<DatabaseComponents<BlockchainT, StateT>>,
            >,
            <StateT as State>::Error,
        >,
        <StateT as State>::Error,
    >,
    <ChainSpecT as ChainSpec>::HaltReason,
>;

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
    fn notify_frame_end<ContextT: Eip3155TracerGetter>(
        context: &mut ContextT,
        result: &FrameResult,
    ) {
        let eip3155_tracer = context.eip3155_tracer();
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
    ContextT: Eip3155TracerGetter + JournalGetter,
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

/// Helper type for a frame that combines EIP-3155 and raw tracers.
pub type Eip3155AndRawTracersFrame<BlockchainErrorT, ChainSpecT, ContextT, StateErrorT> =
    Eip3155TracerFrame<
        TraceCollectorFrame<
            <ChainSpecT as RuntimeSpec>::EvmFrame<
                BlockchainErrorT,
                ContextT,
                InspectableInstructionProvider<
                    ContextT,
                    EthInterpreter,
                    <ChainSpecT as RuntimeSpec>::EvmInstructionProvider<ContextT>,
                >,
                <ChainSpecT as RuntimeSpec>::EvmPrecompileProvider<
                    BlockchainErrorT,
                    ContextT,
                    StateErrorT,
                >,
                StateErrorT,
            >,
            <ChainSpecT as ChainSpec>::HaltReason,
        >,
        <ChainSpecT as ChainSpec>::HaltReason,
    >;
