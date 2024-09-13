use std::sync::Arc;

use edr_generic::GenericChainSpec;
use edr_napi_core::{
    logger::{self, Logger},
    provider::{self, ProviderBuilder, SyncProviderFactory},
    spec::SyncNapiSpec as _,
    subscription,
};
use napi_derive::napi;

use crate::provider::ProviderFactory;

pub struct GenericChainProviderFactory;

impl SyncProviderFactory for GenericChainProviderFactory {
    fn create_provider_builder(
        &self,
        env: &napi::Env,
        provider_config: edr_napi_core::provider::Config,
        logger_config: logger::Config,
        subscription_config: subscription::Config,
    ) -> napi::Result<Box<dyn provider::Builder>> {
        let logger = Logger::<GenericChainSpec>::new(logger_config)?;

        let provider_config =
            edr_provider::ProviderConfig::<GenericChainSpec>::from(provider_config);

        let subscription_callback =
            subscription::Callback::new(env, subscription_config.subscription_callback)?;

        Ok(Box::new(ProviderBuilder::new(
            Box::new(logger),
            provider_config,
            subscription_callback,
        )))
    }
}

#[napi]
pub const GENERIC_CHAIN_TYPE: &str = GenericChainSpec::CHAIN_TYPE;

#[napi]
pub fn generic_chain_provider_factory() -> ProviderFactory {
    let factory: Arc<dyn SyncProviderFactory> = Arc::new(GenericChainProviderFactory);
    factory.into()
}
