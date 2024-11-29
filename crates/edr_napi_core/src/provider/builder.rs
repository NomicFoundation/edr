use std::sync::Arc;

use edr_evm::blockchain::BlockchainErrorForChainSpec;
use edr_provider::{time::CurrentTime, SyncLogger};
use napi::tokio::runtime;

use crate::{provider::SyncProvider, spec::SyncNapiSpec, subscription};

/// A builder for creating a new provider.
pub trait Builder: Send {
    /// Consumes the builder and returns a new provider.
    fn build(self: Box<Self>, runtime: runtime::Handle) -> napi::Result<Arc<dyn SyncProvider>>;
}

pub struct ProviderBuilder<ChainSpecT: SyncNapiSpec> {
    logger:
        Box<dyn SyncLogger<ChainSpecT, BlockchainError = BlockchainErrorForChainSpec<ChainSpecT>>>,
    provider_config: edr_provider::ProviderConfig<ChainSpecT::Hardfork>,
    subscription_callback: subscription::Callback<ChainSpecT>,
}

impl<ChainSpecT: SyncNapiSpec> ProviderBuilder<ChainSpecT> {
    /// Constructs a new instance.
    pub fn new(
        logger: Box<
            dyn SyncLogger<ChainSpecT, BlockchainError = BlockchainErrorForChainSpec<ChainSpecT>>,
        >,
        provider_config: edr_provider::ProviderConfig<ChainSpecT::Hardfork>,
        subscription_callback: subscription::Callback<ChainSpecT>,
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
            builder.logger,
            Box::new(move |event| builder.subscription_callback.call(event)),
            builder.provider_config,
            CurrentTime,
        )
        .map_err(|error| napi::Error::new(napi::Status::GenericFailure, error.to_string()))?;

        Ok(Arc::new(provider))
    }
}
