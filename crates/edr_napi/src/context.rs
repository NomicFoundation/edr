use std::{io, sync::Arc};

use edr_eth::HashMap;
use napi::{
    tokio::{runtime, sync::Mutex as AsyncMutex},
    Env, JsFunction, JsObject, Status,
};
use napi_derive::napi;
use tracing_subscriber::{prelude::*, EnvFilter, Registry};

use crate::{
    logger::{Logger, LoggerConfig},
    provider::{Provider, ProviderConfig, ProviderFactory, SyncProvider, SyncProviderFactory},
    subscribe::SubscriberCallback,
};

#[napi]
pub struct EdrContext {
    inner: Arc<AsyncMutex<Context>>,
}

#[napi]
impl EdrContext {
    #[doc = "Creates a new [`EdrContext`] instance. Should only be called once!"]
    #[napi(constructor)]
    pub fn new() -> napi::Result<Self> {
        let context =
            Context::new().map_err(|e| napi::Error::new(Status::GenericFailure, e.to_string()))?;

        Ok(Self {
            inner: Arc::new(AsyncMutex::new(context)),
        })
    }

    #[doc = "Constructs a new provider with the provided configuration."]
    #[napi(ts_return_type = "Promise<Provider>")]
    pub fn create_provider(
        &self,
        env: Env,
        chain_type: String,
        config: ProviderConfig,
        logger_config: LoggerConfig,
        #[napi(ts_arg_type = "(event: SubscriptionEvent) => void")] subscriber_callback: JsFunction,
    ) -> napi::Result<JsObject> {
        let logger = Logger::new(&env, logger_config)?;
        let subscriber_callback = SubscriberCallback::new(&env, subscriber_callback)?;

        let runtime = runtime::Handle::current();
        let context = self.inner.clone();

        let (deferred, promise) = env.create_deferred()?;
        runtime.clone().spawn_blocking(move || {
            let context = runtime.block_on(async { context.lock().await });

            // #[cfg(feature = "scenarios")]
            // let scenario_file =
            //     runtime::Handle::current().block_on(crate::scenarios::scenario_file(
            //         &config,
            //         edr_provider::Logger::is_enabled(&*logger),
            //     ))?;

            let result = context
                .create_provider(
                    runtime.clone(),
                    &chain_type,
                    config,
                    logger,
                    subscriber_callback,
                )
                .map(|provider| Provider::new(provider, runtime));

            deferred.resolve(|_env| result)
        });

        Ok(promise)
    }

    #[doc = "Registers a new provider factory for the provided chain type."]
    pub async fn register_provider_factory(
        &self,
        chain_type: String,
        factory: ProviderFactory,
    ) -> napi::Result<()> {
        let mut context = self.inner.lock().await;
        context.register_provider_factory(chain_type, factory.as_inner().clone());
        Ok(())
    }
}

pub struct Context {
    provider_factories: HashMap<String, Arc<dyn SyncProviderFactory>>,
    _subscriber_guard: tracing::subscriber::DefaultGuard,
    #[cfg(feature = "tracing")]
    _tracing_write_guard: tracing_flame::FlushGuard<std::io::BufWriter<std::fs::File>>,
}

impl Context {
    /// Creates a new [`Context`] instance. Should only be called once!
    pub fn new() -> io::Result<Self> {
        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_file(true)
            .with_line_number(true)
            .with_thread_ids(true)
            .with_target(false)
            .with_level(true)
            .with_filter(EnvFilter::from_default_env());

        let subscriber = Registry::default().with(fmt_layer);

        #[cfg(feature = "tracing")]
        let (flame_layer, guard) = {
            let (flame_layer, guard) =
                tracing_flame::FlameLayer::with_file("tracing.folded").unwrap();

            let flame_layer = flame_layer.with_empty_samples(false);
            (flame_layer, guard)
        };

        #[cfg(feature = "tracing")]
        let subscriber = subscriber.with(flame_layer);

        let subscriber_guard = tracing::subscriber::set_default(subscriber);

        Ok(Self {
            provider_factories: HashMap::new(),
            _subscriber_guard: subscriber_guard,
            #[cfg(feature = "tracing")]
            _tracing_write_guard: guard,
        })
    }

    /// Registers a new provider factory for the provided chain type.
    pub fn register_provider_factory(
        &mut self,
        chain_type: String,
        factory: Arc<dyn SyncProviderFactory>,
    ) {
        self.provider_factories.insert(chain_type, factory);
    }

    /// Tries to create a new provider for the provided chain type and configuration.
    pub fn create_provider(
        &self,
        runtime: runtime::Handle,
        chain_type: &str,
        config: ProviderConfig,
        logger: Logger,
        subscriber_callback: SubscriberCallback,
    ) -> napi::Result<Arc<dyn SyncProvider>> {
        if let Some(factory) = self.provider_factories.get(chain_type) {
            // #[cfg(feature = "scenarios")]
            // let scenario_file = crate::scenarios::scenario_file(
            //     &config,
            //     edr_provider::Logger::is_enabled(&*logger),
            // )
            // .await?;

            factory.create_provider(runtime, config, logger, subscriber_callback)
        } else {
            Err(napi::Error::new(
                napi::Status::GenericFailure,
                "Provider for provided chain type does not exist",
            ))
        }
    }
}
