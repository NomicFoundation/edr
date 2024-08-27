use std::sync::Arc;

use napi_derive::napi;

use crate::{logger::LoggerConfig, provider, subscription::SubscriptionConfig};

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

/// Trait for creating a new provider using the builder pattern.
pub trait SyncProviderFactory: Send + Sync {
    /// Creates a `ProviderBuilder` that.
    fn create_provider_builder(
        &self,
        env: &napi::Env,
        provider_config: edr_napi_core::provider::Config,
        logger_config: LoggerConfig,
        subscription_config: SubscriptionConfig,
    ) -> napi::Result<Box<dyn provider::Builder>>;
}
