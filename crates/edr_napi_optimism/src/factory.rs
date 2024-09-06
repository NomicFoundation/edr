use edr_napi_core::{
    logger::{self, Logger},
    provider::{self, ProviderBuilder, SyncProviderFactory},
    subscription,
};
use edr_optimism::OptimismChainSpec;

pub struct OptimismProviderFactory;

impl SyncProviderFactory for OptimismProviderFactory {
    fn create_provider_builder(
        &self,
        env: &napi::Env,
        provider_config: provider::Config,
        logger_config: logger::Config,
        subscription_config: subscription::Config,
    ) -> napi::Result<Box<dyn provider::Builder>> {
        let logger = Logger::<OptimismChainSpec>::new(logger_config)?;

        let provider_config =
            edr_provider::ProviderConfig::<OptimismChainSpec>::from(provider_config);

        let subscription_callback =
            subscription::Callback::new(env, subscription_config.subscription_callback)?;

        Ok(Box::new(ProviderBuilder::new(
            Box::new(logger),
            provider_config,
            subscription_callback,
        )))
    }
}
