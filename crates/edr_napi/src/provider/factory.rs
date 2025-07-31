use std::sync::Arc;

use edr_napi_core::provider::SyncProviderFactory;
use edr_provider::time::CurrentTime;
use napi_derive::napi;

#[napi]
pub struct ProviderFactory {
    inner: Arc<dyn SyncProviderFactory<CurrentTime>>,
}

impl ProviderFactory {
    /// Returns a reference to the inner provider factory.
    pub fn as_inner(&self) -> &Arc<dyn SyncProviderFactory<CurrentTime>> {
        &self.inner
    }
}

impl From<Arc<dyn SyncProviderFactory<CurrentTime>>> for ProviderFactory {
    fn from(inner: Arc<dyn SyncProviderFactory<CurrentTime>>) -> Self {
        Self { inner }
    }
}
