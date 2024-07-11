use std::{fmt::Debug, sync::Arc};

use edr_evm::{db::Database, evm::EvmHandler};
use revm_precompile::secp256r1;

/// Registers custom precompiles.
pub fn register_precompiles_handles<const ENABLE_RIP_7212: bool, DatabaseT, ContextT>(
    handler: &mut EvmHandler<'_, ContextT, DatabaseT>,
) where
    DatabaseT: Database,
    DatabaseT::Error: Debug,
{
    let old_handle = handler.pre_execution.load_precompiles();
    handler.pre_execution.load_precompiles = Arc::new(move || {
        let mut precompiles = old_handle.clone();
        if ENABLE_RIP_7212 {
            precompiles.extend([
                // EIP-7212: secp256r1 P256verify
                secp256r1::P256VERIFY,
            ]);
        }

        precompiles
    });
}
