use std::sync::Arc;

use edr_napi_core::provider::SyncProviderFactory as _;
use edr_solidity::contract_decoder::ContractDecoder;
use napi::{bindgen_prelude::BigInt, tokio::runtime, Env};
use napi_derive::napi;

use crate::{
    cast::TryCast as _,
    chains::generic::GenericChainProviderFactory,
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
    let logger_config = logger_config.resolve(&env)?;

    // TODO: https://github.com/NomicFoundation/edr/issues/760
    let build_info_config = edr_solidity::artifacts::BuildInfoConfig::parse_from_buffers(
        (&edr_napi_core::solidity::config::TracingConfigWithBuffers::from(tracing_config)).into(),
    )
    .map_err(|error| napi::Error::from_reason(error.to_string()))?;

    let contract_decoder = ContractDecoder::new(&build_info_config)
        .map_err(|error| napi::Error::from_reason(error.to_string()))?;

    let contract_decoder = Arc::new(contract_decoder);

    let builder = GenericChainProviderFactory.create_provider_builder(
        &env,
        provider_config,
        logger_config,
        subscription_config.into(),
        Arc::clone(&contract_decoder),
    )?;

    let provider = builder
        .build(runtime.clone(), Arc::clone(&time.inner))
        .map_err(|error| napi::Error::from_reason(error.to_string()))?;

    Ok(Provider::new(
        provider,
        runtime,
        contract_decoder,
        #[cfg(feature = "scenarios")]
        None,
    ))
}
