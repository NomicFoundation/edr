use std::sync::Arc;

use napi::tokio::runtime;

use super::SyncProvider;

/// A builder for creating a new provider.
pub trait Builder: Send {
    /// Consumes the builder and returns a new provider.
    fn build(self: Box<Self>, runtime: runtime::Handle) -> napi::Result<Arc<dyn SyncProvider>>;
}
