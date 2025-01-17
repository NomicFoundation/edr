use std::marker::PhantomData;

use crate::spec::ContextForChainSpec;

// /// Type for registering handles, specialised for EDR database component
// types. pub type HandleRegister<'evm, ChainSpecT, BlockchainErrorT,
// DebugDataT, StateT> =     revm::handler::register::HandleRegister<
//         <ChainSpecT as RuntimeSpec>::EvmWiring<
//             WrapDatabaseRef<
//                 DatabaseComponents<
//                     &'evm dyn SyncBlockchain<
//                         ChainSpecT,
//                         BlockchainErrorT,
//                         <StateT as State>::Error,
//                     >,
//                     StateT,
//                 >,
//             >,
//             DebugDataT,
//         >,
//     >;

/// Type for encapsulating contextual data and handler registration in an
/// `EvmBuilder`.
pub struct EvmExtension<ConstructorT, InnerContextT, OuterContextT>
where
    ConstructorT: Fn(InnerContextT) -> OuterContextT,
{
    pub context_constructor: ConstructorT,
    // /// The handler
    // pub handler: HandlerT,
    phantom: PhantomData<(InnerContextT, OuterContextT)>,
}

impl<ConstructorT, InnerContextT, OuterContextT>
    EvmExtension<ConstructorT, InnerContextT, OuterContextT>
where
    ConstructorT: Fn(InnerContextT) -> OuterContextT,
{
    /// Creates a new instance.
    pub fn new(context_constructor: ConstructorT) -> Self {
        Self {
            context_constructor,
            phantom: PhantomData,
        }
    }
}

pub type NoopContextConstructor<BlockchainT, ChainSpecT, StateT> =
    fn(
        ContextForChainSpec<BlockchainT, ChainSpecT, StateT>,
    ) -> ContextForChainSpec<BlockchainT, ChainSpecT, StateT>;
