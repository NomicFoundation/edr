use std::sync::Arc;

use napi::tokio::runtime;
use napi_derive::napi;

use crate::{logger::Logger, subscribe::SubscriberCallback};

use super::{config::ProviderConfig, SyncProvider};

pub trait SyncProviderFactory: Send + Sync {
    fn create_provider(
        &self,
        runtime: runtime::Handle,
        config: ProviderConfig,
        logger: Logger,
        subscriber_callback: SubscriberCallback,
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
