use std::sync::Arc;

use edr_solidity::contract_decoder::ContractDecoder;
use parking_lot::RwLock;
use tokio::runtime;

use crate::{
    logger,
    provider::{self, SyncProvider},
    subscription,
};

/// Trait for creating a new provider.
pub trait SyncProviderFactory: Send + Sync {
    /// Creates a new provider.
    fn create_provider(
        &self,
        runtime: runtime::Handle,
        provider_config: provider::Config,
        logger_config: logger::Config,
        subscription_callback: subscription::Callback,
        contract_decoder: Arc<RwLock<ContractDecoder>>,
    ) -> napi::Result<Arc<dyn SyncProvider>>;
}
