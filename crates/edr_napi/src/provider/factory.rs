use std::sync::Arc;

use edr_napi_core::provider::SyncProvider;
use edr_solidity::contract_decoder::ContractDecoder;
use napi::tokio::runtime;
use napi_derive::napi;
use parking_lot::RwLock;

use crate::subscription::SubscriptionTsfn;

/// Trait for creating a new provider.
pub trait SyncProviderFactory: Send + Sync {
    /// Creates a new provider.
    fn create_provider(
        &self,
        runtime: runtime::Handle,
        provider_config: edr_napi_core::provider::Config,
        logger_config: edr_napi_core::logger::Config,
        subscription_callback: Arc<SubscriptionTsfn>,
        contract_decoder: Arc<RwLock<ContractDecoder>>,
    ) -> napi::Result<Arc<dyn SyncProvider>>;
}

#[napi]
pub struct ProviderFactory {
    inner: Arc<dyn SyncProviderFactory>,
}

impl ProviderFactory {
    /// Returns a reference to the inner provider factory.
    pub fn as_inner(&self) -> &Arc<dyn SyncProviderFactory> {
        &self.inner
    }
}

impl From<Arc<dyn SyncProviderFactory>> for ProviderFactory {
    fn from(inner: Arc<dyn SyncProviderFactory>) -> Self {
        Self { inner }
    }
}
