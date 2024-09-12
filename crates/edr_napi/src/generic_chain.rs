use std::sync::Arc;

use edr_generic::GenericChainSpec;
use napi_derive::napi;

use crate::{
    logger::{Logger, LoggerConfig},
    provider::{self, factory::SyncProviderFactory, ProviderBuilder, ProviderFactory},
    spec::SyncNapiSpec,
    subscription::{SubscriptionCallback, SubscriptionConfig},
};

pub struct GenericChainProviderFactory;

impl SyncProviderFactory for GenericChainProviderFactory {
    fn create_provider_builder(
        &self,
        env: &napi::Env,
        provider_config: edr_napi_core::provider::Config,
        logger_config: LoggerConfig,
        subscription_config: SubscriptionConfig,
    ) -> napi::Result<Box<dyn provider::Builder>> {
        let logger = Logger::<GenericChainSpec>::new(env, logger_config)?;

        let provider_config =
            edr_provider::ProviderConfig::<GenericChainSpec>::from(provider_config);

        let subscription_callback =
            SubscriptionCallback::new(env, subscription_config.subscription_callback)?;

        Ok(Box::new(ProviderBuilder::new(
            logger,
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
