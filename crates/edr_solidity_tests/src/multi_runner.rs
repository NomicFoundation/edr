//! Forge test runner for multiple contracts.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    marker::PhantomData,
    path::PathBuf,
    sync::Arc,
    time::Instant,
};

use alloy_json_abi::JsonAbi;
use alloy_primitives::Bytes;
use derive_more::Debug;
use derive_where::derive_where;
use edr_artifact::ArtifactId;
use edr_chain_spec::{EvmHaltReason, HaltReasonTrait};
use edr_coverage::{reporter::SyncOnCollectedCoverageCallback, CodeCoverageReporter};
use edr_decoder_revert::RevertDecoder;
use edr_solidity::{config::IncludeTraces, contract_decoder::SyncNestedTraceDecoder};
use eyre::Result;
use foundry_cheatcodes::TestFunctionIdentifier;
use foundry_evm::{
    backend::Predeploy,
    contracts::ContractsByArtifact,
    evm_context::{
        BlockEnvTr, ChainContextTr, EvmBuilderTrait, EvmEnv, HardforkTr, TransactionEnvTr,
        TransactionErrorTrait,
    },
    executors::ExecutorBuilder,
    fork::CreateFork,
    inspectors::{cheatcodes::CheatsConfigOptions, CheatsConfig},
    opts::EvmOpts,
    traces::{identifier::TraceIdentifiers, CallTraceDecoderBuilder, TracingMode},
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::{
    config::CollectStackTraces,
    contracts::get_contract_name,
    error::TestRunnerError,
    fuzz::{invariant::InvariantConfig, FuzzConfig},
    inline_config::{self, InlineConfigRoot, SharedInlineConfigProvider},
    result::{SuiteResult, SuiteRunOutcome, TestRunOutcome},
    runner::{ContractRunnerArtifacts, ContractRunnerOptions},
    ContractRunner, SolidityTestRunnerConfig, SolidityTestRunnerConfigError, TestFilter,
    TestFunctionConfigOverride,
};

pub struct SuiteResultAndArtifactId<HaltReasonT> {
    pub artifact_id: ArtifactId,
    pub result: SuiteResult<HaltReasonT>,
}

/// A deployable test contract
#[derive(Debug, Clone)]
pub struct TestContract {
    /// The test contract abi
    pub abi: JsonAbi,
    /// The test contract bytecode
    pub bytecode: Bytes,
}

pub trait OnTestSuiteCompletedFn<HaltReasonT>:
    Fn(SuiteResultAndArtifactId<HaltReasonT>) + Send + Sync
{
}

impl<FnT, HaltReasonT> OnTestSuiteCompletedFn<HaltReasonT> for FnT where
    FnT: Fn(SuiteResultAndArtifactId<HaltReasonT>) + Send + Sync
{
}

pub type TestContracts = BTreeMap<ArtifactId, TestContract>;

pub struct SolidityTestResult {
    pub gas_report: Option<edr_gas_report::GasReport>,
}

pub struct SolidityTestsRunResult<HaltReasonT> {
    pub test_result: SolidityTestResult,
    pub suite_results: BTreeMap<String, SuiteResult<HaltReasonT>>,
}

/// A multi contract runner receives a set of contracts deployed in an EVM
/// instance and proceeds to run all test functions in these contracts.
#[derive_where(Clone; BlockT, HardforkT, NestedTraceDecoderT, TransactionT)]
#[derive(Debug)]
pub struct MultiContractRunner<
    BlockT: BlockEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TransactionT>,
    HaltReasonT: HaltReasonTrait,
    HardforkT: HardforkTr,
    NestedTraceDecoderT,
    TransactionErrorT: TransactionErrorTrait,
    TransactionT: TransactionEnvTr,
> {
    /// The project root directory.
    project_root: PathBuf,
    /// Test contracts to deploy
    test_contracts: TestContracts,
    /// Known contracts by artifact id
    known_contracts: Arc<ContractsByArtifact>,
    /// Libraries to deploy.
    libs_to_deploy: Vec<Bytes>,
    /// Provides contract metadata from calldata and traces.
    contract_decoder: Arc<NestedTraceDecoderT>,
    /// Cheats config.
    cheats_config_options: Arc<CheatsConfigOptions>,
    /// The EVM instance used in the test runner
    evm_opts: EvmOpts<HardforkT>,
    /// The configured evm
    env: EvmEnv<BlockT, TransactionT, HardforkT>,
    /// The local predeploys
    local_predeploys: Vec<Predeploy>,
    /// Revert decoder. Contains all known errors and their selectors.
    revert_decoder: RevertDecoder,
    /// The fork to use at launch
    fork: Option<CreateFork<BlockT, TransactionT, HardforkT>>,
    /// Whether to collect stack traces.
    collect_stack_traces: CollectStackTraces,
    /// Whether to collect coverage info
    coverage: bool,
    /// Whether to enable trace mode and which traces to include in test
    /// results.
    include_traces: IncludeTraces,
    /// Whether to enable Solidity fuzz fixtures support
    enable_fuzz_fixtures: bool,
    /// Whether to enable table test support
    enable_table_tests: bool,
    fuzz_config: FuzzConfig,
    invariant_config: InvariantConfig,
    /// Optionally, a callback to be called when coverage is collected.
    #[debug(skip)]
    on_collected_coverage_fn: Option<Box<dyn SyncOnCollectedCoverageCallback>>,
    /// Whether to generate a gas report after running the tests.
    generate_gas_report: bool,
    /// Collects and serves the inline configuration parsed from test sources.
    inline_config_provider: SharedInlineConfigProvider,
    #[allow(clippy::type_complexity)]
    _phantom: PhantomData<fn() -> (ChainContextT, EvmBuilderT, HaltReasonT, TransactionErrorT)>,
}

impl<
        BlockT: BlockEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<
            BlockT,
            ChainContextT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            TransactionT,
        >,
        HaltReasonT: 'static + HaltReasonTrait + TryInto<EvmHaltReason> + Send + Sync,
        HardforkT: HardforkTr,
        NestedTraceDecoderT: SyncNestedTraceDecoder<HaltReasonT>,
        TransactionErrorT: TransactionErrorTrait,
        TransactionT: TransactionEnvTr,
    >
    MultiContractRunner<
        BlockT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        NestedTraceDecoderT,
        TransactionErrorT,
        TransactionT,
    >
{
    /// Creates a new multi contract runner.
    pub async fn new(
        config: SolidityTestRunnerConfig<HardforkT>,
        test_contracts: TestContracts,
        known_contracts: ContractsByArtifact,
        libs_to_deploy: Vec<Bytes>,
        contract_decoder: NestedTraceDecoderT,
        revert_decoder: RevertDecoder,
    ) -> Result<Self, SolidityTestRunnerConfigError> {
        let env = config
            .evm_opts
            .evm_env()
            .await
            .map_err(SolidityTestRunnerConfigError::EvmEnv)?;

        let fork = config.get_fork().await?;

        let SolidityTestRunnerConfig {
            collect_stack_traces,
            mut include_traces,
            coverage,
            mut evm_opts,
            project_root,
            cheats_config_options,
            fuzz,
            mut invariant,
            enable_fuzz_fixtures,
            enable_table_tests,
            local_predeploys,
            on_collected_coverage_fn,
            generate_gas_report,
            test_source_paths,
            import_resolver,
        } = config;

        // Collect the test sources' inline configuration up front, off the async
        // runtime (it reads and parses files). Any problem found — reported per
        // test function, each located at its source line — fails here, aborting
        // the whole run before any test executes.
        let roots = inline_config_roots(&test_source_paths, &test_contracts);
        let inline_config_provider = tokio::task::spawn_blocking(move || {
            SharedInlineConfigProvider::collect(roots, import_resolver)
        })
        .await
        .expect("Thread shouldn't panic");
        inline_config_provider
            .validate()
            .map_err(SolidityTestRunnerConfigError::InlineConfig)?;

        // Do canonicalization in blocking context.
        // Canonicalization can touch the file system, hence the blocking thread
        let project_root = tokio::task::spawn_blocking(move || {
            dunce::canonicalize(project_root)
                .map_err(SolidityTestRunnerConfigError::InvalidProjectRoot)
        })
        .await
        .expect("Thread shouldn't panic")?;

        if generate_gas_report {
            // Traces are needed to generate a gas report
            include_traces = IncludeTraces::All;
            // Enable EVM isolation for more accurate gas measurements
            evm_opts.isolate = true;
        } else {
            // Nothing consumes gas-report samples, so don't collect them.
            invariant.gas_report_samples = 0;
        }

        Ok(Self {
            project_root,
            test_contracts,
            known_contracts: Arc::new(known_contracts),
            contract_decoder: Arc::new(contract_decoder),
            libs_to_deploy,
            cheats_config_options: Arc::new(cheats_config_options),
            evm_opts,
            env,
            local_predeploys,
            revert_decoder,
            fork,
            collect_stack_traces,
            coverage,
            include_traces,
            enable_fuzz_fixtures,
            enable_table_tests,
            fuzz_config: fuzz,
            invariant_config: invariant,
            on_collected_coverage_fn,
            _phantom: PhantomData,
            generate_gas_report,
            inline_config_provider,
        })
    }

    /// Returns the known contracts.
    pub fn known_contracts(&self) -> &ContractsByArtifact {
        &self.known_contracts
    }

    /// Returns an iterator over all contracts that match the filter.
    fn matching_contracts<'a>(
        &'a self,
        filter: &'a dyn TestFilter,
    ) -> impl Iterator<Item = (&'a ArtifactId, &'a TestContract)> {
        self.test_contracts
            .iter()
            .filter(|&(id, _)| matches_contract(id, filter))
    }
}

/// The inline configuration a test suite runs with, extracted from its
/// contract's source (see
/// [`MultiContractRunner::inline_config_overrides`]).
struct SuiteInlineConfig {
    /// The merged per-function configuration overrides.
    overrides: HashMap<TestFunctionIdentifier, TestFunctionConfigOverride>,
    /// The functions that opted into `allowInternalExpectRevert`.
    allow_internal_expect_revert: HashSet<TestFunctionIdentifier>,
    /// Warnings for directives that cannot take effect, e.g. on a function
    /// that matches nothing in the contract ABI. Reported on the suite's
    /// result.
    warnings: Vec<String>,
}

impl<
        BlockT: BlockEnvTr,
        ChainContextT: 'static + ChainContextTr + Send + Sync,
        EvmBuilderT: 'static
            + EvmBuilderTrait<
                BlockT,
                ChainContextT,
                HaltReasonT,
                HardforkT,
                TransactionErrorT,
                TransactionT,
            >,
        HaltReasonT: 'static + HaltReasonTrait + TryInto<EvmHaltReason> + Send + Sync,
        HardforkT: HardforkTr,
        NestedTraceDecoderT: SyncNestedTraceDecoder<HaltReasonT>,
        TransactionErrorT: TransactionErrorTrait,
        TransactionT: TransactionEnvTr,
    >
    MultiContractRunner<
        BlockT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        NestedTraceDecoderT,
        TransactionErrorT,
        TransactionT,
    >
{
    /// Parses the inline configuration of the given test contract from its
    /// source, returning the overrides keyed by test function selector, the
    /// set of tests that opted into `allowInternalExpectRevert`, and warnings
    /// for directives that cannot take effect.
    ///
    /// A contract-level configuration (NatSpec above the contract definition)
    /// applies to every test function in the contract's ABI — including
    /// inherited ones — with function-level directives taking per-key
    /// precedence.
    ///
    /// Returns empty collections when the contract's source isn't available or
    /// carries no inline configuration. Malformed directives never reach here:
    /// they are caught up front by [`SharedInlineConfigProvider::validate`],
    /// which fails runner creation (see [`Self::new`]).
    fn inline_config_overrides(
        &self,
        artifact_id: &ArtifactId,
        contract: &TestContract,
    ) -> SuiteInlineConfig {
        let parsed = self
            .inline_config_provider
            .get(&artifact_id.source, &artifact_id.name);

        let mut warnings = Vec::new();

        // Key the overrides by selector: every overload is a distinct test
        // with a distinct selector.
        let mut by_selector: HashMap<String, TestFunctionConfigOverride> = HashMap::new();
        for function_override in parsed.functions {
            let mut matched = false;
            for function in contract
                .abi
                .functions()
                .filter(|function| function.name == function_override.function_name)
            {
                matched = true;
                by_selector.insert(
                    function.selector().to_string(),
                    function_override.config.clone(),
                );
            }
            // A name matching no ABI function (e.g. not externally callable)
            // can't be run as a test, so its override would silently do
            // nothing; warn instead.
            if !matched {
                warnings.push(format!(
                    "Found inline configuration for function \"{}\" in contract \"{}\", but no \
                     matching function exists in the contract ABI (it may not be externally \
                     callable), so it will not run as a test and its configuration is ignored.",
                    function_override.function_name, artifact_id.name,
                ));
            }
        }

        // Apply the contract-level configuration underneath every test
        // function's own overrides. Walking the ABI (rather than the source)
        // covers inherited test functions too.
        if let Some(contract_config) = &parsed.contract {
            for function in contract.abi.functions() {
                if !inline_config::is_test_function(&function.name) {
                    continue;
                }
                by_selector
                    .entry(function.selector().to_string())
                    .or_default()
                    .fill_unset_from(contract_config);
            }
        }

        let mut overrides = HashMap::new();
        let mut allow_internal_expect_revert = HashSet::new();

        for (function_selector, config) in by_selector {
            let identifier = TestFunctionIdentifier {
                contract_artifact: artifact_id.clone(),
                function_selector,
            };
            if config.allow_internal_expect_revert == Some(true) {
                allow_internal_expect_revert.insert(identifier.clone());
            }
            overrides.insert(identifier, config);
        }

        SuiteInlineConfig {
            overrides,
            allow_internal_expect_revert,
            warnings,
        }
    }

    fn run_test_suite(
        &self,
        artifact_id: &ArtifactId,
        contract: &TestContract,
        fork: Option<CreateFork<BlockT, TransactionT, HardforkT>>,
        filter: &dyn TestFilter,
        handle: &tokio::runtime::Handle,
    ) -> Result<
        (
            SuiteResult<HaltReasonT>,
            Option<crate::gas_report::GasReport>,
        ),
        TestRunnerError,
    > {
        let identifier = artifact_id.identifier();
        let mut span_name = identifier.as_str();

        if !enabled!(tracing::Level::TRACE) {
            span_name = get_contract_name(&identifier);
        }
        let span = debug_span!("suite", name = %span_name);
        let span_local = span.clone();
        let _guard = span_local.enter();

        debug!("start executing all tests in contract");

        // Extract per-test inline configuration from the contract's source.
        let SuiteInlineConfig {
            overrides: inline_overrides,
            allow_internal_expect_revert,
            warnings: inline_config_warnings,
        } = self.inline_config_overrides(artifact_id, contract);

        let cheats_config = CheatsConfig::new(
            self.project_root.clone(),
            (*self.cheats_config_options).clone(),
            self.evm_opts.clone(),
            self.known_contracts.clone(),
            Some(artifact_id.clone()),
            allow_internal_expect_revert,
        );

        let tracing_mode = match self.collect_stack_traces {
            CollectStackTraces::Always => TracingMode::WithSteps,
            CollectStackTraces::OnFailure => match self.include_traces {
                IncludeTraces::Failing | IncludeTraces::All => TracingMode::WithoutSteps,
                IncludeTraces::None => TracingMode::None,
            },
        };

        let executor_builder =
            ExecutorBuilder::<BlockT, TransactionT, HardforkT, ChainContextT>::new()
                .env(self.env.clone())
                .fork(fork)
                .gas_limit(self.evm_opts.gas_limit())
                .inspectors(|stack| {
                    stack
                        .cheatcodes(Arc::new(cheats_config))
                        .trace(tracing_mode)
                        .code_coverage(
                            self.on_collected_coverage_fn
                                .clone()
                                .map(CodeCoverageReporter::new),
                        )
                        .coverage(self.coverage)
                        .enable_isolation(self.evm_opts.isolate)
                })
                .spec(self.evm_opts.spec)
                .local_predeploys(self.local_predeploys.clone());

        if !enabled!(tracing::Level::TRACE) {
            span_name = &artifact_id.name;
        }
        let _guard = info_span!("run_tests", name = span_name).entered();

        debug!("start executing all tests in contract");

        let runner: ContractRunner<'_, _, _, EvmBuilderT, HaltReasonT, _, _, _, _> =
            ContractRunner::new(
                &identifier,
                artifact_id,
                executor_builder,
                contract,
                ContractRunnerArtifacts {
                    revert_decoder: &self.revert_decoder,
                    known_contracts: &self.known_contracts,
                    libs_to_deploy: &self.libs_to_deploy,
                    contract_decoder: Arc::clone(&self.contract_decoder),
                    _phantom: PhantomData,
                },
                ContractRunnerOptions {
                    initial_balance: self.evm_opts.initial_balance,
                    sender: self.evm_opts.sender,
                    enable_fuzz_fixtures: self.enable_fuzz_fixtures,
                    enable_table_tests: self.enable_table_tests,
                    fuzz_config: &self.fuzz_config,
                    invariant_config: &self.invariant_config,
                    test_function_overrides: &inline_overrides,
                    generate_gas_report: self.generate_gas_report,
                    include_traces: self.include_traces,
                },
                span,
            );
        let SuiteRunOutcome {
            duration,
            mut setup_traces,
            test_outcomes,
            mut warnings,
        } = runner.run_tests(filter, handle)?;
        warnings.extend(inline_config_warnings);

        let mut gas_report = self
            .generate_gas_report
            .then(crate::gas_report::GasReport::default);

        let test_results = if self.include_traces == IncludeTraces::None {
            test_outcomes
                .into_iter()
                .map(|(signature, outcome)| (signature, outcome.result))
                .collect()
        } else {
            let mut decoder = CallTraceDecoderBuilder::new().build();
            let mut trace_identifier = TraceIdentifiers::new().with_local(&self.known_contracts);

            // Setup traces are shared across all tests in the suite, so decode and analyze
            // them only once.
            tokio::task::block_in_place(|| {
                handle.block_on(
                    setup_traces.identify_and_decode(&mut decoder, &mut trace_identifier),
                );
            });

            if let Some(gas_report) = gas_report.as_mut() {
                tokio::task::block_in_place(|| {
                    handle.block_on(
                        gas_report.analyze(setup_traces.iter().map(|(_, a)| &a.arena), &decoder),
                    );
                });
            }

            test_outcomes
                .into_iter()
                .map(|(signature, outcome)| {
                    let TestRunOutcome {
                        mut result,
                        gas_report_samples,
                    } = outcome;

                    if self
                        .include_traces
                        .should_include(|| result.status.is_failure())
                    {
                        decoder.clear_addresses();
                        decoder.labels.extend(
                            result
                                .labeled_addresses
                                .iter()
                                .map(|(k, v)| (*k, v.clone())),
                        );

                        // Re-execute setup traces to collect identities of deployed contracts.
                        for (_, arena) in &setup_traces {
                            decoder.identify(arena, &mut trace_identifier);
                        }

                        tokio::task::block_in_place(|| {
                            handle.block_on(
                                result
                                    .execution_traces
                                    .identify_and_decode(&mut decoder, &mut trace_identifier),
                            );
                        });

                        if let Some(gas_report) = gas_report.as_mut() {
                            tokio::task::block_in_place(|| {
                                handle.block_on(gas_report.analyze(
                                    result.execution_traces.iter().map(|arena| &arena.arena),
                                    &decoder,
                                ));
                            });

                            for trace in gas_report_samples.into_iter().flatten() {
                                decoder.clear_addresses();

                                // Re-execute setup traces to collect identities of deployed
                                // contracts.
                                for (_, arena) in &setup_traces {
                                    decoder.identify(arena, &mut trace_identifier);
                                }

                                for arena in trace {
                                    decoder.identify(&arena, &mut trace_identifier);
                                    tokio::task::block_in_place(|| {
                                        handle.block_on(gas_report.analyze([&arena], &decoder));
                                    });
                                }
                            }
                        }
                    }

                    (signature, result)
                })
                .collect()
        };

        let r = SuiteResult::new(duration, setup_traces, test_results, warnings);
        debug!(duration=?r.duration, "executed all tests in contract");

        Ok((r, gas_report))
    }

    /// Executes _all_ tests that match the given `filter`.
    ///
    /// The same as [`test`](Self::test), but returns the results instead of
    /// streaming them.
    ///
    /// Note that this method returns only when all tests have been executed.
    pub async fn test_collect(
        self,
        filter: impl TestFilter + 'static,
    ) -> SolidityTestsRunResult<HaltReasonT> {
        let (tx_results, mut rx_results) =
            tokio::sync::mpsc::unbounded_channel::<SuiteResultAndArtifactId<HaltReasonT>>();

        let test_result = self
            .test(
                tokio::runtime::Handle::current(),
                Arc::new(filter),
                Arc::new(move |suite_result| {
                    let _ = tx_results.clone().send(suite_result);
                }),
                // TODO return error instead once testsa are backported
            )
            .expect("fork created successfully");

        let mut suite_results = BTreeMap::new();

        while let Some(SuiteResultAndArtifactId {
            artifact_id,
            result,
        }) = rx_results.recv().await
        {
            suite_results.insert(artifact_id.identifier(), result);
        }

        SolidityTestsRunResult {
            test_result,
            suite_results,
        }
    }

    /// Executes _all_ tests that match the given `filter`.
    ///
    /// The method _blocks_ until all test suites have completed. The result of
    /// each test suite is sent back via the callback function as soon as it's
    /// completed.
    ///
    /// This will create the runtime based on the configured `evm` ops and
    /// create the `Backend` before executing all contracts and their tests
    /// in _parallel_.
    ///
    /// Each Executor gets its own instance of the `Backend`.
    pub fn test(
        mut self,
        tokio_handle: tokio::runtime::Handle,
        filter: Arc<impl TestFilter + 'static>,
        on_test_suite_completed_fn: Arc<dyn OnTestSuiteCompletedFn<HaltReasonT>>,
    ) -> Result<SolidityTestResult, TestRunnerError> {
        trace!("running all tests");

        let fork = self.fork.take();

        let find_timer = Instant::now();
        let contracts = self
            .matching_contracts(filter.as_ref())
            .map(|(id, contract)| (id.clone(), contract.clone()))
            .collect::<Vec<_>>();
        let find_time = find_timer.elapsed();
        debug!(
            "Found {} test contracts out of {} in {:?}",
            contracts.len(),
            self.test_contracts.len(),
            find_time,
        );

        // Gas reports are collected for each suite and merged at the end to allow
        // parallel execution of test suites.
        let gas_reports = contracts
            .into_par_iter()
            .map(|(id, contract)| {
                let _guard = tokio_handle.enter();
                let (result, gas_report) = self.run_test_suite(
                    &id,
                    &contract,
                    fork.clone(),
                    filter.as_ref(),
                    &tokio_handle,
                )?;

                on_test_suite_completed_fn(SuiteResultAndArtifactId {
                    artifact_id: id,
                    result,
                });

                Ok::<_, TestRunnerError>(gas_report)
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Merge gas reports
        let gas_report = self.generate_gas_report.then(|| {
            gas_reports
                .into_iter()
                .flatten()
                .map(edr_gas_report::GasReport::from)
                .fold(edr_gas_report::GasReport::default(), |mut acc, report| {
                    acc.merge(report);
                    acc
                })
        });

        Ok(SolidityTestResult { gas_report })
    }
}

fn matches_contract(id: &ArtifactId, filter: &dyn TestFilter) -> bool {
    filter.matches_path(&id.source) && filter.matches_contract(&id.name)
}

/// Builds the inline-config roots for every test contract whose source has a
/// known on-disk path, deduplicated by source (a source declaring multiple test
/// contracts is parsed once).
fn inline_config_roots(
    test_source_paths: &HashMap<PathBuf, PathBuf>,
    test_contracts: &TestContracts,
) -> Vec<InlineConfigRoot> {
    let mut roots_by_source = HashMap::new();
    for artifact_id in test_contracts.keys() {
        if let Some(path) = test_source_paths.get(&artifact_id.source) {
            roots_by_source
                .entry(artifact_id.source.clone())
                .or_insert_with(|| InlineConfigRoot {
                    source: artifact_id.source.clone(),
                    path: path.clone(),
                    version: artifact_id.version.clone(),
                });
        }
    }
    roots_by_source.into_values().collect()
}
