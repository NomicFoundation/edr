use auto_impl::auto_impl;

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
pub struct DebugContext<ContextT, DebugDataT, HandlerT> {
    /// The inner context
    pub context: ContextT,
    /// The contextual data.
    pub data: DebugDataT,
    /// The handler
    pub handler: HandlerT,
}
