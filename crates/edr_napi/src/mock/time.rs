use std::sync::Arc;

use edr_generic::GenericChainSpec;
use edr_napi_core::logger::Logger;
use edr_solidity::contract_decoder::ContractDecoder;
use napi::{bindgen_prelude::BigInt, tokio::runtime, Env};
use napi_derive::napi;

use crate::{
    cast::TryCast as _,
    config::{ProviderConfig, TracingConfigWithBuffers},
    logger::LoggerConfig,
    provider::Provider,
    subscription::SubscriptionConfig,
};

#[napi]
pub struct MockTime {
    inner: Arc<edr_provider::time::MockTime>,
}

#[napi]
impl MockTime {
    #[doc = "Creates a new instance of `MockTime` with the current time."]
    #[napi(factory)]
    pub fn now() -> Self {
        Self {
            inner: Arc::new(edr_provider::time::MockTime::now()),
        }
    }

    #[doc = "Adds the specified number of seconds to the current time."]
    #[napi]
    pub fn add_seconds(&self, seconds: BigInt) -> napi::Result<()> {
        let seconds = seconds.try_cast()?;

        self.inner.add_seconds(seconds);
        Ok(())
    }
}

#[doc = "Creates a provider with a mock timer."]
#[doc = "For testing purposes."]
#[napi]
pub fn create_provider_with_mock_timer(
    env: Env,
    provider_config: ProviderConfig,
    logger_config: LoggerConfig,
    subscription_config: SubscriptionConfig,
    tracing_config: TracingConfigWithBuffers,
    time: &MockTime,
) -> napi::Result<Provider> {
    let runtime = runtime::Handle::current();

    let provider_config = provider_config.resolve(&env, runtime.clone())?;
    let provider_config =
        edr_provider::ProviderConfig::<edr_eth::l1::SpecId>::try_from(provider_config)?;

    let logger_config = logger_config.resolve(&env)?;

    // TODO: https://github.com/NomicFoundation/edr/issues/760
    let build_info_config = edr_solidity::artifacts::BuildInfoConfig::parse_from_buffers(
        (&edr_napi_core::solidity::config::TracingConfigWithBuffers::from(tracing_config)).into(),
    )
    .map_err(|error| napi::Error::from_reason(error.to_string()))?;

    let contract_decoder = ContractDecoder::new(&build_info_config)
        .map_err(|error| napi::Error::from_reason(error.to_string()))?;

    let contract_decoder = Arc::new(contract_decoder);

    let logger = Logger::<GenericChainSpec, Arc<edr_provider::time::MockTime>>::new(
        logger_config,
        Arc::clone(&contract_decoder),
    )?;

    let subscription_config = edr_napi_core::subscription::Config::from(subscription_config);
    let subscription_callback = edr_napi_core::subscription::Callback::new(
        &env,
        subscription_config.subscription_callback,
    )?;

    let provider =
        edr_provider::Provider::<GenericChainSpec, Arc<edr_provider::time::MockTime>>::new(
            runtime.clone(),
            Box::new(logger),
            Box::new(move |event| subscription_callback.call(event)),
            provider_config,
            contract_decoder.clone(),
            Arc::clone(&time.inner),
        )
        .map_err(|error| napi::Error::from_reason(error.to_string()))?;

    Ok(Provider::new(
        Arc::new(provider),
        runtime,
        contract_decoder,
        #[cfg(feature = "scenarios")]
        None,
    ))
}
