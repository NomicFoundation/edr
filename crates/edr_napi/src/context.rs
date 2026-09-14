use std::sync::Arc;

use edr_decoder_revert::RevertDecoder;
use edr_napi_core::{provider::SyncProvider, solidity};
use edr_primitives::HashMap;
use edr_solidity_tests::{
    multi_runner::{SuiteResultAndArtifactId, TestContract, TestContracts},
    TestFilterConfig,
};
use napi::{
    bindgen_prelude::{Function, Object},
    threadsafe_function::{ThreadsafeCallContext, ThreadsafeFunctionCallMode},
    tokio::{runtime, sync::Mutex as AsyncMutex},
    Env,
};
use napi_derive::napi;
use tracing_subscriber::{prelude::*, EnvFilter, Registry};

use crate::{
    async_deallocator::AsyncDeallocator,
    config::{resolve_configs, ConfigResolution, ProviderConfig, TracingConfigWithBuffers},
    contract_decoder::ContractDecoder,
    logger::LoggerConfig,
    provider::{factory::SyncProviderFactory, GcProvider, Provider, ProviderFactory},
    solidity_tests::{
        artifact::{Artifact, ArtifactId, TestSuiteReference},
        artifact_contracts_from_napi,
        config::SolidityTestRunnerConfigArgs,
        factory::SolidityTestRunnerFactory,
        inline_config, load_project_inputs,
        test_results::{SolidityTestResult, SuiteResult},
        ArtifactContracts, LinkingOutput, ProjectInputs,
    },
    subscription::SubscriptionConfig,
};

/// The result of a Solidity test run, distinguishing an inline-config failure —
/// which we surface as a structured JS error built on the JS thread — from a
/// completed run.
enum RunOutcome {
    Completed(edr_solidity_tests::multi_runner::SolidityTestResult),
    InvalidInlineConfig(edr_solidity_tests::inline_config::InlineConfigErrors),
}

/// Unwraps `$expr`, or rejects `$deferred` with the error and returns
/// `Ok($promise)` from the enclosing function.
macro_rules! try_or_reject_promise {
    ($deferred:ident, $promise:ident, $expr:expr) => {
        match $expr {
            Ok(value) => value,
            Err(error) => {
                $deferred.reject(error);
                return Ok($promise);
            }
        }
    };
}

#[napi]
pub struct EdrContext {
    inner: Arc<AsyncMutex<Context>>,
}

#[napi]
impl EdrContext {
    /// Creates a new [`EdrContext`] instance. Should only be called once!
    #[napi(catch_unwind, constructor, async_runtime)]
    pub fn new() -> napi::Result<Self> {
        let context = Context::new(runtime::Handle::current())?;

        Ok(Self {
            inner: Arc::new(AsyncMutex::new(context)),
        })
    }

    /// Constructs a new provider with the provided configuration.
    #[napi(catch_unwind, async_runtime, ts_return_type = "Promise<Provider>")]
    pub fn create_provider<'env>(
        &self,
        env: &'env Env,
        chain_type: String,
        provider_config: ProviderConfig<'env>,
        logger_config: LoggerConfig<'env>,
        subscription_config: SubscriptionConfig<'env>,
        contract_decoder: &ContractDecoder,
    ) -> napi::Result<Object<'env>> {
        let (deferred, promise) = env.create_deferred()?;

        let runtime = runtime::Handle::current();

        let ConfigResolution {
            logger_config,
            provider_config,
            subscription_callback,
        } = try_or_reject_promise!(
            deferred,
            promise,
            resolve_configs(
                runtime.clone(),
                provider_config,
                logger_config,
                subscription_config,
            )
        );

        #[cfg(feature = "scenarios")]
        let scenario_file = try_or_reject_promise!(
            deferred,
            promise,
            runtime.clone().block_on(crate::scenarios::scenario_file(
                chain_type.clone(),
                provider_config.clone(),
                logger_config.enable,
            ))
        );

        let (factory, dropped_provider_sender) = {
            // TODO: https://github.com/NomicFoundation/edr/issues/760
            // TODO: Don't block the JS event loop
            let context = runtime.block_on(async { self.inner.lock().await });

            let factory = try_or_reject_promise!(
                deferred,
                promise,
                context.get_provider_factory(&chain_type)
            );
            let dropped_provider_sender = context.provider_deallocator.sender();

            (factory, dropped_provider_sender)
        };

        let contract_decoder = Arc::clone(contract_decoder.as_inner());
        runtime.clone().spawn_blocking(move || {
            let result = factory
                .create_provider(
                    runtime.clone(),
                    provider_config,
                    logger_config,
                    subscription_callback,
                    Arc::clone(&contract_decoder),
                )
                .map(|provider| {
                    GcProvider::from(Provider::new(
                        provider,
                        runtime,
                        contract_decoder,
                        dropped_provider_sender,
                        #[cfg(feature = "scenarios")]
                        scenario_file,
                    ))
                });

            deferred.resolve(|_env| result);
        });

        Ok(promise)
    }

    /// Registers a new provider factory for the provided chain type.
    #[napi(catch_unwind)]
    pub async fn register_provider_factory(
        &self,
        chain_type: String,
        factory: &ProviderFactory,
    ) -> napi::Result<()> {
        let mut context = self.inner.lock().await;
        context.register_provider_factory(chain_type, factory.as_inner().clone());
        Ok(())
    }

    #[napi(catch_unwind)]
    pub async fn register_solidity_test_runner_factory(
        &self,
        chain_type: String,
        factory: &SolidityTestRunnerFactory,
    ) -> napi::Result<()> {
        let mut context = self.inner.lock().await;
        context.register_solidity_test_runner(chain_type, factory.as_inner().clone());
        Ok(())
    }

    /// Executes Solidity tests
    ///
    /// The function will return a promise that resolves to a
    /// [`SolidityTestResult`].
    ///
    /// Arguments:
    /// - `chainType`: the same chain type that was passed to
    ///   `registerProviderFactory`.
    /// - `artifacts`: the project's compilation output artifacts. It's
    ///   important to include include all artifacts here, otherwise cheatcodes
    ///   that access artifacts and other functionality (e.g. auto-linking, gas
    ///   reports) can break.
    /// - `testSuites`: the test suite ids that specify which test suites to
    ///   execute. The test suite artifacts must be present in `artifacts`.
    /// - `configArgs`: solidity test runner configuration. See the struct docs
    ///   for details.
    /// - `tracingConfig`: the build infos used for stack trace generation.
    ///   These are lazily parsed and it's important that they're passed as
    ///   Uint8 arrays for performance.
    /// - `onTestSuiteCompletedCallback`: The progress callback will be called
    ///   with the results of each test suite as soon as it finished executing.
    #[allow(clippy::too_many_arguments)]
    #[napi(
        catch_unwind,
        async_runtime,
        ts_return_type = "Promise<SolidityTestResult>"
    )]
    pub fn run_solidity_tests<'env>(
        &self,
        env: &'env Env,
        chain_type: String,
        artifacts: Vec<Artifact>,
        test_suites: Vec<ArtifactId>,
        config_args: SolidityTestRunnerConfigArgs<'env>,
        tracing_config: TracingConfigWithBuffers,
        on_test_suite_completed_callback: Function<'env, SuiteResult, ()>,
    ) -> napi::Result<Object<'env>> {
        let (deferred, promise) = env.create_deferred()?;

        let on_test_suite_completed = try_or_reject_promise!(
            deferred,
            promise,
            build_on_test_suite_completed(on_test_suite_completed_callback)
        );

        let test_filter: Arc<TestFilterConfig> = Arc::new(try_or_reject_promise!(
            deferred,
            promise,
            config_args.try_get_test_filter()
        ));

        let runtime = runtime::Handle::current();
        let config =
            try_or_reject_promise!(deferred, promise, config_args.resolve(runtime.clone()));

        let context = self.inner.clone();
        runtime.clone().spawn(async move {
            let result = async {
                let factory = {
                    let context = context.lock().await;
                    context.solidity_test_runner_factory(&chain_type).await?
                };

                let artifact_contracts = artifact_contracts_from_napi(artifacts)?;

                let test_suites = test_suites
                    .into_iter()
                    .map(edr_artifact::ArtifactId::try_from)
                    .collect::<Result<Vec<_>, _>>()?;

                run_test_suites(
                    runtime,
                    factory,
                    config,
                    artifact_contracts,
                    test_suites,
                    edr_napi_core::solidity::config::TracingConfigWithBuffers::from(tracing_config),
                    test_filter,
                    on_test_suite_completed,
                )
                .await
            }
            .await;

            match result {
                Ok(outcome) => deferred.resolve(move |env| match outcome {
                    RunOutcome::Completed(test_result) => Ok(SolidityTestResult::from(test_result)),
                    RunOutcome::InvalidInlineConfig(errors) => {
                        Err(inline_config::to_napi_error(&env, &errors))
                    }
                }),
                Err(error) => deferred.reject(error),
            }
        });

        Ok(promise)
    }

    /// Executes Solidity tests, loading artifacts and build infos from the
    /// provided artifact directories.
    ///
    /// The function will return a promise that resolves to a
    /// [`SolidityTestResult`].
    ///
    /// Arguments:
    /// - `chainType`: the same chain type that was passed to
    ///   `registerProviderFactory`.
    /// - `artifactsDirectories`: the paths of the project's artifact
    ///   directories, in the Hardhat v3 format. All artifacts are loaded, so
    ///   that cheatcodes that access artifacts and other functionality (e.g.
    ///   auto-linking, gas reports) work.
    /// - `testSuites`: references to the test suite contracts to execute. The
    ///   referenced artifacts must be present in the artifact directories.
    /// - `configArgs`: solidity test runner configuration. See the struct docs
    ///   for details.
    /// - `onTestSuiteCompletedCallback`: The progress callback will be called
    ///   with the results of each test suite as soon as it finished executing.
    #[napi(
        catch_unwind,
        async_runtime,
        ts_return_type = "Promise<SolidityTestResult>"
    )]
    pub fn run_solidity_tests_from_paths<'env>(
        &self,
        env: &'env Env,
        chain_type: String,
        artifacts_directories: Vec<String>,
        test_suites: Vec<TestSuiteReference>,
        config_args: SolidityTestRunnerConfigArgs<'env>,
        on_test_suite_completed_callback: Function<'env, SuiteResult, ()>,
    ) -> napi::Result<Object<'env>> {
        let (deferred, promise) = env.create_deferred()?;

        let on_test_suite_completed = try_or_reject_promise!(
            deferred,
            promise,
            build_on_test_suite_completed(on_test_suite_completed_callback)
        );

        let test_filter: Arc<TestFilterConfig> = Arc::new(try_or_reject_promise!(
            deferred,
            promise,
            config_args.try_get_test_filter()
        ));

        let runtime = runtime::Handle::current();
        let config =
            try_or_reject_promise!(deferred, promise, config_args.resolve(runtime.clone()));

        let context = self.inner.clone();
        runtime.clone().spawn(async move {
            let result = async {
                let factory = {
                    let context = context.lock().await;
                    context.solidity_test_runner_factory(&chain_type).await?
                };

                let ProjectInputs {
                    artifact_contracts,
                    test_suites,
                    build_infos,
                } = runtime
                    .spawn_blocking(move || {
                        load_project_inputs(&artifacts_directories, test_suites)
                    })
                    .await
                    .expect("Failed to join artifact loading thread")?;

                let tracing_config = edr_napi_core::solidity::config::TracingConfigWithBuffers {
                    build_infos: Some(napi::Either::B(build_infos)),
                    // Skipping `Ignored*`-named contracts is only used by EDR's own test harnesses;
                    // production Hardhat always passes `false`.
                    ignore_contracts: Some(false),
                };

                run_test_suites(
                    runtime,
                    factory,
                    config,
                    artifact_contracts,
                    test_suites,
                    tracing_config,
                    test_filter,
                    on_test_suite_completed,
                )
                .await
            }
            .await;

            match result {
                Ok(outcome) => deferred.resolve(move |env| match outcome {
                    RunOutcome::Completed(test_result) => Ok(SolidityTestResult::from(test_result)),
                    RunOutcome::InvalidInlineConfig(errors) => {
                        Err(inline_config::to_napi_error(&env, &errors))
                    }
                }),
                Err(error) => deferred.reject(error),
            }
        });

        Ok(promise)
    }
}

/// Builds a callback that forwards each completed test suite's results to the
/// provided JS function.
fn build_on_test_suite_completed(
    on_test_suite_completed_callback: Function<'_, SuiteResult, ()>,
) -> napi::Result<impl Fn(SuiteResult) + Send + Sync + 'static> {
    let on_test_suite_completed_callback = on_test_suite_completed_callback
        .build_threadsafe_function::<SuiteResult>()
        .build_callback(|ctx: ThreadsafeCallContext<SuiteResult>| Ok(ctx.value))?;

    Ok(move |suite_result: SuiteResult| {
        let status = on_test_suite_completed_callback
            .call(suite_result, ThreadsafeFunctionCallMode::Blocking);

        // This should always succeed since we're using an unbounded queue.
        // We add an assertion for completeness.
        assert_eq!(
            status,
            napi::Status::Ok,
            "Failed to call on_test_suite_completed_callback with status: {status}"
        );
    })
}

/// Links the provided artifacts and runs the provided test suites,
/// forwarding each suite's results to `on_test_suite_completed` as soon as it
/// finished executing.
///
/// Returns a [`RunOutcome`] so that inline-config failures can be converted
/// into a structured JS error by the caller on the JS thread.
#[allow(clippy::too_many_arguments)]
async fn run_test_suites(
    runtime: runtime::Handle,
    factory: Arc<dyn solidity::SyncTestRunnerFactory>,
    config: edr_napi_core::solidity::config::TestRunnerConfig,
    artifact_contracts: ArtifactContracts,
    test_suites: Vec<edr_artifact::ArtifactId>,
    tracing_config: edr_napi_core::solidity::config::TracingConfigWithBuffers,
    test_filter: Arc<TestFilterConfig>,
    on_test_suite_completed: impl Fn(SuiteResult) + Send + Sync + 'static,
) -> napi::Result<RunOutcome> {
    let linking_output = LinkingOutput::link(&config.project_root, artifact_contracts)?;

    // Build revert decoder from ABIs of all artifacts.
    let abis = linking_output
        .known_contracts
        .iter()
        .map(|(_, contract)| &contract.abi);

    let revert_decoder = RevertDecoder::new().with_abis(abis);

    let contracts = test_suites
        .iter()
        .map(|artifact_id| {
            let contract_data =
                linking_output
                    .known_contracts
                    .get(artifact_id)
                    .ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::GenericFailure,
                            format!("Unknown contract: {}", artifact_id.identifier()),
                        )
                    })?;

            let bytecode = contract_data.bytecode.clone().ok_or_else(|| {
                napi::Error::new(
                    napi::Status::GenericFailure,
                    format!(
                        "No bytecode for test suite contract: {}",
                        artifact_id.identifier()
                    ),
                )
            })?;

            let test_contract = TestContract {
                abi: contract_data.abi.clone(),
                bytecode,
            };

            Ok((artifact_id.clone(), test_contract))
        })
        .collect::<napi::Result<TestContracts>>()?;

    let include_traces = config.include_traces.into();

    let runtime_for_factory = runtime.clone();
    let create_result = runtime
        .clone()
        .spawn_blocking(move || {
            factory.create_test_runner(
                runtime_for_factory,
                config,
                contracts,
                linking_output.known_contracts,
                linking_output.libs_to_deploy,
                revert_decoder,
                tracing_config,
            )
        })
        .await
        .expect("Failed to join test runner factory thread");

    let test_runner = match create_result {
        // An inline-config failure carries structured, located problems that
        // are surfaced on the rejected promise's error as `inlineConfigErrors`.
        // Building that JS object requires the JS thread, so it's routed to
        // the caller's deferred resolver (which runs there) rather than being
        // rejected here, which would only carry a message.
        Err(solidity::CreateTestRunnerError::InvalidInlineConfig(errors)) => {
            return Ok(RunOutcome::InvalidInlineConfig(errors));
        }
        Err(solidity::CreateTestRunnerError::Failed(error)) => {
            return Err(error);
        }
        Ok(test_runner) => test_runner,
    };

    let runtime_for_runner = runtime.clone();
    let test_result = runtime
        .spawn_blocking(move || {
            test_runner.run_tests(
                runtime_for_runner,
                test_filter,
                Arc::new(
                    move |SuiteResultAndArtifactId {
                              artifact_id,
                              result,
                          }| {
                        let suite_result = SuiteResult::new(artifact_id, result, include_traces);

                        on_test_suite_completed(suite_result);
                    },
                ),
            )
        })
        .await
        .expect("Failed to join test runner thread")?;

    Ok(RunOutcome::Completed(test_result))
}

#[cfg(feature = "test-mock")]
#[napi]
impl EdrContext {
    /// Creates a mock provider, which always returns the given response.
    /// For testing purposes.
    // `GcProvider` only carries the value to JavaScript, which receives a
    // `Provider`.
    #[napi(async_runtime, ts_return_type = "Provider")]
    pub fn create_mock_provider(
        &self,
        mocked_response: serde_json::Value,
    ) -> napi::Result<GcProvider> {
        use crate::mock::MockProvider;

        let runtime = runtime::Handle::current();

        let dropped_provider_sender = {
            let context = runtime.block_on(async { self.inner.lock().await });
            context.provider_deallocator.sender()
        };

        Ok(GcProvider::from(Provider::new(
            Arc::new(MockProvider::new(mocked_response)?),
            runtime,
            Arc::default(),
            dropped_provider_sender,
            #[cfg(feature = "scenarios")]
            None,
        )))
    }

    /// Creates a provider with a mock timer.
    /// For testing purposes.
    #[napi(catch_unwind, async_runtime, ts_return_type = "Promise<Provider>")]
    pub fn create_provider_with_mock_timer<'env>(
        &self,
        env: &'env Env,
        provider_config: ProviderConfig<'env>,
        logger_config: LoggerConfig<'env>,
        subscription_config: SubscriptionConfig<'env>,
        contract_decoder: &ContractDecoder,
        time: &crate::mock::time::MockTime,
    ) -> napi::Result<Object<'env>> {
        use edr_generic::GenericChainSpec;
        use edr_napi_core::logger::Logger;

        let (deferred, promise) = env.create_deferred()?;

        let runtime = runtime::Handle::current();

        let ConfigResolution {
            logger_config,
            provider_config,
            subscription_callback,
        } = try_or_reject_promise!(
            deferred,
            promise,
            resolve_configs(
                runtime.clone(),
                provider_config,
                logger_config,
                subscription_config,
            )
        );

        let contract_decoder = Arc::clone(contract_decoder.as_inner());
        let timer = Arc::clone(time.as_inner());

        let dropped_provider_sender = {
            let context = runtime.block_on(async { self.inner.lock().await });
            context.provider_deallocator.sender()
        };

        runtime.clone().spawn_blocking(move || {
            // Using a closure to limit the scope, allowing us to use `?` for error
            // handling. This is necessary because the result of the closure is used
            // to resolve the deferred promise.
            let create_provider = move || -> napi::Result<GcProvider> {
                use crate::subscription::subscriber_callback_for_chain_spec;

                let logger = Logger::<GenericChainSpec, Arc<edr_provider::time::MockTime>>::new(
                    logger_config,
                    Arc::clone(&contract_decoder),
                )?;

                let provider_config =
                    edr_provider::config::Provider::<edr_chain_l1::Hardfork>::try_from(
                        provider_config,
                    )?;

                let provider = edr_provider::Provider::<
                    GenericChainSpec,
                    Arc<edr_provider::time::MockTime>,
                >::new(
                    runtime.clone(),
                    Box::new(logger),
                    subscriber_callback_for_chain_spec::<
                        GenericChainSpec,
                        Arc<edr_provider::time::MockTime>,
                    >(subscription_callback),
                    provider_config,
                    Arc::clone(&contract_decoder),
                    timer,
                )
                .map_err(|error| napi::Error::from_reason(error.to_string()))?;

                Ok(GcProvider::from(Provider::new(
                    Arc::new(provider),
                    runtime,
                    contract_decoder,
                    dropped_provider_sender,
                    #[cfg(feature = "scenarios")]
                    None,
                )))
            };

            let result = create_provider();
            deferred.resolve(|_env| result);
        });

        Ok(promise)
    }
}

pub struct Context {
    provider_factories: HashMap<String, Arc<dyn SyncProviderFactory>>,
    solidity_test_runner_factories: HashMap<String, Arc<dyn solidity::SyncTestRunnerFactory>>,
    provider_deallocator: AsyncDeallocator<Arc<dyn SyncProvider>>,
    #[cfg(feature = "tracing")]
    _tracing_write_guard: tracing_flame::FlushGuard<std::io::BufWriter<std::fs::File>>,
}

impl Context {
    /// Creates a new [`Context`] instance. Should only be called once!
    pub fn new(runtime: runtime::Handle) -> napi::Result<Self> {
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
            let (flame_layer, guard) = tracing_flame::FlameLayer::with_file("tracing.folded")
                .map_err(|err| {
                    napi::Error::new(
                        napi::Status::GenericFailure,
                        format!("Failed to create tracing.folded file with error: {err:?}"),
                    )
                })?;

            let flame_layer = flame_layer.with_empty_samples(false);
            (flame_layer, guard)
        };

        #[cfg(feature = "tracing")]
        let subscriber = subscriber.with(flame_layer);

        if let Err(error) = tracing::subscriber::set_global_default(subscriber) {
            println!(
                "Failed to set global tracing subscriber with error: {error}\n\
                Please only initialize EdrContext once per process to avoid this error."
            );
        }

        Ok(Self {
            provider_factories: HashMap::default(),
            solidity_test_runner_factories: HashMap::default(),
            provider_deallocator: AsyncDeallocator::new(runtime).map_err(|error| {
                napi::Error::new(
                    napi::Status::GenericFailure,
                    format!("Failed to spawn the provider deallocator thread: {error}"),
                )
            })?,
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

    pub fn register_solidity_test_runner(
        &mut self,
        chain_type: String,
        factory: Arc<dyn solidity::SyncTestRunnerFactory>,
    ) {
        self.solidity_test_runner_factories
            .insert(chain_type, factory);
    }

    /// Tries to create a new provider for the provided chain type and
    /// configuration.
    pub fn get_provider_factory(
        &self,
        chain_type: &str,
    ) -> napi::Result<Arc<dyn SyncProviderFactory>> {
        if let Some(factory) = self.provider_factories.get(chain_type) {
            Ok(Arc::clone(factory))
        } else {
            Err(napi::Error::new(
                napi::Status::GenericFailure,
                "Provider for provided chain type does not exist",
            ))
        }
    }

    pub async fn solidity_test_runner_factory(
        &self,
        chain_type: &str,
    ) -> napi::Result<Arc<dyn solidity::SyncTestRunnerFactory>> {
        if let Some(factory) = self.solidity_test_runner_factories.get(chain_type) {
            Ok(Arc::clone(factory))
        } else {
            Err(napi::Error::new(
                napi::Status::GenericFailure,
                "Solidity test runner for provided chain type does not exist",
            ))
        }
    }
}
