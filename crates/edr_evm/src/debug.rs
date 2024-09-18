use auto_impl::auto_impl;
use edr_eth::result::InvalidTransaction;
use revm::{
    db::{DatabaseComponents, StateRef, WrapDatabaseRef},
    primitives::TransactionValidation,
};

use crate::{blockchain::SyncBlockchain, chain_spec::RuntimeSpec};

/// Type for registering handles, specialised for EDR database component types.
pub type HandleRegister<'evm, ChainSpecT, BlockchainErrorT, DebugDataT, StateT> =
    revm::handler::register::HandleRegister<
        <ChainSpecT as RuntimeSpec>::EvmWiring<
            WrapDatabaseRef<
                DatabaseComponents<
                    StateT,
                    &'evm dyn SyncBlockchain<
                        ChainSpecT,
                        BlockchainErrorT,
                        <StateT as StateRef>::Error,
                    >,
                >,
            >,
            DebugDataT,
        >,
    >;

/// Type for encapsulating contextual data and handler registration in an
/// `EvmBuilder`.
pub struct DebugContext<'evm, ChainSpecT, BlockchainErrorT, DebugDataT, StateT>
where
    ChainSpecT:
        RuntimeSpec<Transaction: TransactionValidation<ValidationError: From<InvalidTransaction>>>,
    StateT: StateRef,
{
    /// The contextual data.
    pub data: DebugDataT,
    /// The function to register handles.
    pub register_handles_fn: HandleRegister<'evm, ChainSpecT, BlockchainErrorT, DebugDataT, StateT>,
}

pub struct EvmContext<'evm, ChainSpecT, BlockchainErrorT, DebugDataT, StateT>
where
    ChainSpecT:
        RuntimeSpec<Transaction: TransactionValidation<ValidationError: From<InvalidTransaction>>>,
    StateT: StateRef,
{
    pub debug: Option<DebugContext<'evm, ChainSpecT, BlockchainErrorT, DebugDataT, StateT>>,
    pub state: StateT,
}

/// Trait for getting contextual data.
#[auto_impl(&mut)]
pub trait GetContextData<DataT> {
    /// Retrieves the contextual data.
    fn get_context_data(&mut self) -> &mut DataT;
}
