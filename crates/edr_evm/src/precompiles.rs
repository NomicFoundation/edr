use std::{fmt::Debug, sync::Arc};

use edr_eth::{Address, HashMap};
use revm::{db::Database, ContextPrecompile, EvmHandler};

/// Registers custom precompiles.
pub fn register_precompiles_handles<ChainSpecT, DatabaseT, ContextT>(
    handler: &mut EvmHandler<'_, ChainSpecT, ContextT, DatabaseT>,
    precompiles: HashMap<Address, ContextPrecompile<ChainSpecT, DatabaseT>>,
) where
    ChainSpecT: revm::ChainSpec,
    DatabaseT: Database<Error: Debug>,
{
    let old_handle = handler.pre_execution.load_precompiles();
    handler.pre_execution.load_precompiles = Arc::new(move || {
        let mut new_handle = old_handle.clone();

        new_handle.extend(precompiles.clone());

        new_handle
    });
}
