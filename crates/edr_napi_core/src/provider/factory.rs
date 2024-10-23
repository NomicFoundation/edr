use crate::{logger, provider, subscription};

/// Trait for creating a new provider using the builder pattern.
pub trait SyncProviderFactory: Send + Sync {
    /// Creates a `ProviderBuilder` that.
    fn create_provider_builder(
        &self,
        env: &napi::Env,
        provider_config: provider::Config,
        logger_config: logger::Config,
        subscription_config: subscription::Config,
    ) -> napi::Result<Box<dyn provider::Builder>>;
}
