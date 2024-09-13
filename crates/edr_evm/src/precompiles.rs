use std::sync::Arc;

use edr_eth::{Address, HashMap};
use revm::{ContextPrecompile, EvmHandler, EvmWiring};

/// Registers custom precompiles.
pub fn register_precompiles_handles<EvmWiringT: EvmWiring>(
    handler: &mut EvmHandler<'_, EvmWiringT>,
    precompiles: HashMap<Address, ContextPrecompile<EvmWiringT>>,
) {
    let old_handle = handler.pre_execution.load_precompiles();
    handler.pre_execution.load_precompiles = Arc::new(move || {
        let mut new_handle = old_handle.clone();

        new_handle.extend(precompiles.clone());

        new_handle
    });
}
