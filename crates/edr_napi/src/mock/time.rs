use std::sync::Arc;

use edr_eth::{spec::ChainSpec, B256};
use edr_evm::spec::RuntimeSpec;
use edr_generic::GenericChainSpec;
use edr_napi_core::logger::Logger;
use edr_rpc_eth::RpcSpec;
use edr_solidity::contract_decoder::ContractDecoder;
use napi::{bindgen_prelude::BigInt, tokio::runtime, Env, JsObject};
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
    #[napi(factory, catch_unwind)]
    pub fn now() -> Self {
        Self {
            inner: Arc::new(edr_provider::time::MockTime::now()),
        }
    }

    #[doc = "Adds the specified number of seconds to the current time."]
    #[napi(catch_unwind)]
    pub fn add_seconds(&self, seconds: BigInt) -> napi::Result<()> {
        let seconds = seconds.try_cast()?;

        self.inner.add_seconds(seconds);
        Ok(())
    }
}

#[doc = "Creates a provider with a mock timer."]
#[doc = "For testing purposes."]
#[napi(catch_unwind, ts_return_type = "Promise<Provider>")]
pub fn create_provider_with_mock_timer(
    env: Env,
    provider_config: ProviderConfig,
    logger_config: LoggerConfig,
    subscription_config: SubscriptionConfig,
    tracing_config: TracingConfigWithBuffers,
    time: &MockTime,
) -> napi::Result<JsObject> {
    let (deferred, promise) = env.create_deferred()?;

    macro_rules! try_or_reject_promise {
        ($expr:expr) => {
            match $expr {
                Ok(value) => value,
                Err(error) => {
                    deferred.reject(error);
                    return Ok(promise);
                }
            }
        };
    }

    let runtime = runtime::Handle::current();
    let provider_config = try_or_reject_promise!(provider_config.resolve(&env, runtime.clone()));

    let logger_config = try_or_reject_promise!(logger_config.resolve(&env));

    // TODO: https://github.com/NomicFoundation/edr/issues/760
    let build_info_config = try_or_reject_promise!(
        edr_solidity::artifacts::BuildInfoConfig::parse_from_buffers(
            (&edr_napi_core::solidity::config::TracingConfigWithBuffers::from(tracing_config))
                .into(),
        )
        .map_err(|error| napi::Error::from_reason(error.to_string()))
    );

    let contract_decoder = try_or_reject_promise!(ContractDecoder::new(&build_info_config)
        .map_or_else(
            |error| Err(napi::Error::from_reason(error.to_string())),
            |contract_decoder| Ok(Arc::new(contract_decoder))
        ));

    let subscription_config = edr_napi_core::subscription::Config::from(subscription_config);
    let subscription_callback = try_or_reject_promise!(edr_napi_core::subscription::Callback::new(
        &env,
        subscription_config.subscription_callback,
    ));

    let logger = try_or_reject_promise!(Logger::<
        GenericChainSpec,
        Arc<edr_provider::time::MockTime>,
    >::new(logger_config, Arc::clone(&contract_decoder),));

    let provider_config = try_or_reject_promise!(
        edr_provider::ProviderConfig::<edr_eth::l1::SpecId>::try_from(provider_config)
    );

    let timer = Arc::clone(&time.inner);

    runtime.clone().spawn_blocking(move || {
        let result =
            edr_provider::Provider::<GenericChainSpec, Arc<edr_provider::time::MockTime>>::new(
                runtime.clone(),
                Box::new(logger),
                Box::new(move |event| {
                    let event = edr_napi_core::subscription::SubscriptionEvent::new::<
                        <GenericChainSpec as RuntimeSpec>::Block,
                        <GenericChainSpec as RpcSpec>::RpcBlock<B256>,
                        <GenericChainSpec as ChainSpec>::SignedTransaction,
                    >(event);

                    subscription_callback.call(event);
                }),
                provider_config,
                contract_decoder.clone(),
                timer,
            )
            .map_or_else(
                |error| Err(napi::Error::from_reason(error.to_string())),
                |provider| {
                    Ok(Provider::new(
                        Arc::new(provider),
                        runtime,
                        contract_decoder,
                        #[cfg(feature = "scenarios")]
                        None,
                    ))
                },
            );

        deferred.resolve(|_env| result);
    });

    Ok(promise)
}
