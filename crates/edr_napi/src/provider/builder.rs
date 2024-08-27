use std::sync::Arc;

use edr_provider::time::CurrentTime;
use napi::tokio::runtime;

use crate::{
    logger::Logger, provider::SyncProvider, spec::SyncNapiSpec, subscription::SubscriptionCallback,
};

/// A builder for creating a new provider.
pub trait Builder: Send {
    /// Consumes the builder and returns a new provider.
    fn build(self: Box<Self>, runtime: runtime::Handle) -> napi::Result<Arc<dyn SyncProvider>>;
}

pub struct ProviderBuilder<ChainSpecT: SyncNapiSpec> {
    logger: Logger<ChainSpecT>,
    provider_config: edr_provider::ProviderConfig<ChainSpecT>,
    subscription_callback: SubscriptionCallback<ChainSpecT>,
}

impl<ChainSpecT: SyncNapiSpec> ProviderBuilder<ChainSpecT> {
    /// Constructs a new instance.
    pub fn new(
        logger: Logger<ChainSpecT>,
        provider_config: edr_provider::ProviderConfig<ChainSpecT>,
        subscription_callback: SubscriptionCallback<ChainSpecT>,
    ) -> Self {
        Self {
            logger,
            provider_config,
            subscription_callback,
        }
    }
}

impl<ChainSpecT: SyncNapiSpec> Builder for ProviderBuilder<ChainSpecT> {
    fn build(self: Box<Self>, runtime: runtime::Handle) -> napi::Result<Arc<dyn SyncProvider>> {
        let builder = *self;

        let provider = edr_provider::Provider::<ChainSpecT>::new(
            runtime.clone(),
            Box::new(builder.logger),
            Box::new(move |event| builder.subscription_callback.call(event)),
            builder.provider_config,
            CurrentTime,
        )
        .map_err(|error| napi::Error::new(napi::Status::GenericFailure, error.to_string()))?;

        Ok(Arc::new(provider))
    }
}
