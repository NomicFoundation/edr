use std::{fmt::Debug, sync::Arc};

use edr_eth::{Address, HashMap};
use revm::ContextPrecompile;

use crate::{db::Database, evm::EvmHandler};

/// Registers custom precompiles.
pub fn register_precompiles_handles<DatabaseT, ContextT>(
    handler: &mut EvmHandler<'_, ContextT, DatabaseT>,
    precompiles: HashMap<Address, ContextPrecompile<DatabaseT>>,
) where
    DatabaseT: Database,
    DatabaseT::Error: Debug,
{
    let old_handle = handler.pre_execution.load_precompiles();
    handler.pre_execution.load_precompiles = Arc::new(move || {
        let mut new_handle = old_handle.clone();

        new_handle.extend(precompiles.clone());

        new_handle
    });
}
