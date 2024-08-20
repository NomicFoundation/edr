use std::sync::Arc;

use edr_eth::chain_spec::L1ChainSpec;
use edr_provider::time::CurrentTime;
use napi::tokio::runtime;
use napi_derive::napi;

use crate::{logger::Logger, subscribe::SubscriberCallback};

use super::{factory::ProviderFactory, ProviderConfig, SyncProvider, SyncProviderFactory};

pub struct L1ProviderFactory;

impl SyncProviderFactory for L1ProviderFactory {
    fn create_provider(
        &self,
        runtime: runtime::Handle,
        config: ProviderConfig,
        logger: Logger,
        subscriber_callback: SubscriberCallback,
    ) -> napi::Result<Arc<dyn SyncProvider>> {
        let config = edr_provider::ProviderConfig::<L1ChainSpec>::try_from(config)?;

        let provider = edr_provider::Provider::<L1ChainSpec>::new(
            runtime.clone(),
            Box::new(logger),
            Box::new(move |event| subscriber_callback.call(event)),
            config,
            CurrentTime,
        )
        .map_err(|error| napi::Error::new(napi::Status::GenericFailure, error.to_string()))?;

        Ok(Arc::new(provider))
    }
}

#[napi]
pub fn l1_provider_factory() -> ProviderFactory {
    let factory: Arc<dyn SyncProviderFactory> = Arc::new(L1ProviderFactory);
    factory.into()
}
