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
use edr_solidity_collector_eip712::collector::Eip712TypeCollection;
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
    traces::{
        decode_trace_arena, identifier::TraceIdentifiers, CallTraceDecoderBuilder, TracingMode,
    },
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::{
    config::CollectStackTraces,
    contracts::get_contract_name,
    error::TestRunnerError,
    fuzz::{invariant::InvariantConfig, FuzzConfig},
    inline_config::{self, InlineConfigErrors},
    result::SuiteResult,
    runner::{ContractRunnerArtifacts, ContractRunnerOptions},
    test_sources::{collect_test_sources, CollectedTestSource, TestSourceRoot},
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
    /// Per suite, the data extracted from its test source at construction:
    /// resolved inline-config overrides and EIP-712 struct definitions.
    suite_source_data: BTreeMap<ArtifactId, SuiteSourceData>,
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
            invariant,
            enable_fuzz_fixtures,
            enable_table_tests,
            local_predeploys,
            on_collected_coverage_fn,
            generate_gas_report,
            test_source_paths,
            import_resolver,
        } = config;

        // Read and parse the test sources up front, off the async runtime.
        // Each unique source is parsed once; both its inline test
        // configuration and its EIP-712 struct definitions are extracted from
        // the same compilation unit. Any inline-config problem found —
        // reported per test function, each located at its source line — fails
        // here, aborting the whole run before any test executes.
        let roots = test_source_roots(&test_source_paths, &test_contracts);
        let (collected_sources, inline_config_errors) =
            tokio::task::spawn_blocking(move || collect_test_sources(&roots, &import_resolver))
                .await
                .expect("Thread shouldn't panic");
        if !inline_config_errors.is_empty() {
            return Err(SolidityTestRunnerConfigError::InlineConfig(
                InlineConfigErrors::new(inline_config_errors),
            ));
        }

        // Attach to each suite the data extracted from its source. Suites
        // whose source wasn't collected (no `test_source_paths` entry) get
        // empty data.
        let suite_source_data = test_contracts
            .iter()
            .map(|(artifact_id, contract)| {
                let data = collected_sources
                    .get(&artifact_id.source)
                    .map(|source| SuiteSourceData::new(source, artifact_id, &contract.abi))
                    .unwrap_or_default();
                (artifact_id.clone(), data)
            })
            .collect();

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
            suite_source_data,
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

        // Fetch the per-test inline configuration and EIP-712 struct
        // definitions extracted from the contract's source at construction.
        let SuiteSourceData {
            test_function_overrides: inline_overrides,
            allow_internal_expect_revert,
            eip712_types,
        } = self
            .suite_source_data
            .get(artifact_id)
            .cloned()
            .unwrap_or_default();

        let cheats_config = CheatsConfig::new(
            self.project_root.clone(),
            (*self.cheats_config_options).clone(),
            self.evm_opts.clone(),
            self.known_contracts.clone(),
            artifact_id.clone(),
            allow_internal_expect_revert,
            eip712_types,
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
                        .cheatcodes(cheats_config)
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
                },
                span,
            );
        let mut r = runner.run_tests(filter, handle)?;

        let mut gas_report = self
            .generate_gas_report
            .then(crate::gas_report::GasReport::default);

        if self.include_traces != IncludeTraces::None {
            let mut decoder = CallTraceDecoderBuilder::new().build();
            let mut trace_identifier = TraceIdentifiers::new().with_local(&self.known_contracts);

            // Setup traces are shared across all tests in the suite, so decode and analyze
            // them only once.
            for (_, arena) in &mut r.setup_traces {
                decoder.identify(arena, &mut trace_identifier);
                tokio::task::block_in_place(|| {
                    handle.block_on(decode_trace_arena(arena, &decoder));
                });
            }

            if let Some(gas_report) = gas_report.as_mut() {
                tokio::task::block_in_place(|| {
                    handle.block_on(
                        gas_report.analyze(r.setup_traces.iter().map(|(_, a)| &a.arena), &decoder),
                    );
                });
            }

            for result in r.test_results.values_mut() {
                if result.status.is_success() && self.include_traces != IncludeTraces::All {
                    continue;
                }

                decoder.clear_addresses();
                decoder.labels.extend(
                    result
                        .labeled_addresses
                        .iter()
                        .map(|(k, v)| (*k, v.clone())),
                );

                // Re-execute setup traces to collect identities of deployed contracts.
                for (_, arena) in &mut r.setup_traces {
                    decoder.identify(arena, &mut trace_identifier);
                }

                for arena in &mut result.execution_traces {
                    decoder.identify(arena, &mut trace_identifier);
                    tokio::task::block_in_place(|| {
                        handle.block_on(decode_trace_arena(arena, &decoder));
                    });
                }

                if let Some(gas_report) = gas_report.as_mut() {
                    tokio::task::block_in_place(|| {
                        handle.block_on(gas_report.analyze(
                            result.execution_traces.iter().map(|arena| &arena.arena),
                            &decoder,
                        ));
                    });

                    for trace in &result.gas_report_traces {
                        decoder.clear_addresses();

                        // Re-execute setup traces to collect identities of deployed contracts.
                        for (_, arena) in &r.setup_traces {
                            decoder.identify(arena, &mut trace_identifier);
                        }

                        for arena in trace {
                            decoder.identify(arena, &mut trace_identifier);
                            tokio::task::block_in_place(|| {
                                handle.block_on(gas_report.analyze([arena], &decoder));
                            });
                        }
                    }
                }
                // Clear memory.
                result.gas_report_traces.clear();
            }
        }
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

/// Builds the collection roots for every test contract whose source has a
/// known on-disk path, deduplicated by source (a source declaring multiple test
/// contracts is parsed once), sorted by source so problems are reported in a
/// deterministic order.
fn test_source_roots(
    test_source_paths: &HashMap<PathBuf, PathBuf>,
    test_contracts: &TestContracts,
) -> Vec<TestSourceRoot> {
    let mut roots_by_source = BTreeMap::new();
    for artifact_id in test_contracts.keys() {
        if let Some(path) = test_source_paths.get(&artifact_id.source) {
            roots_by_source
                .entry(artifact_id.source.clone())
                .or_insert_with(|| TestSourceRoot {
                    source: artifact_id.source.clone(),
                    path: path.clone(),
                    version: artifact_id.version.clone(),
                });
        }
    }
    roots_by_source.into_values().collect()
}

/// The data extracted from a test suite's source at runner construction: its
/// inline configuration resolved against the contract's ABI, and the EIP-712
/// struct definitions served to the `eip712HashType`/`eip712HashStruct`
/// cheatcodes.
#[derive(Clone, Debug, Default)]
struct SuiteSourceData {
    /// Per-test-function config overrides.
    test_function_overrides: HashMap<TestFunctionIdentifier, TestFunctionConfigOverride>,
    /// The test functions that opted into `allowInternalExpectRevert`.
    allow_internal_expect_revert: HashSet<TestFunctionIdentifier>,
    /// The EIP-712 struct definitions reachable from the suite's source.
    eip712_types: Eip712TypeCollection,
}

impl SuiteSourceData {
    /// Builds a suite's data by resolving its source's parsed function
    /// overrides against the contract's ABI and attaching the source's
    /// EIP-712 types.
    ///
    /// A contract that carries no inline configuration yields empty overrides.
    /// Malformed directives never reach here: they are caught during
    /// collection, which fails runner creation (see
    /// [`MultiContractRunner::new`]).
    fn new(source: &CollectedTestSource, artifact_id: &ArtifactId, abi: &JsonAbi) -> Self {
        let parsed = source
            .overrides
            .get(&artifact_id.name)
            .map(Vec::as_slice)
            .unwrap_or_default();

        let mut test_function_overrides = HashMap::new();
        let mut allow_internal_expect_revert = HashSet::new();

        for function_override in parsed {
            let Some(function_selector) =
                inline_config::resolve_selector(abi, &function_override.function_name)
            else {
                // Not part of the ABI (e.g. not externally callable), so it
                // can't be run as a test; ignore it.
                continue;
            };
            let identifier = TestFunctionIdentifier {
                contract_artifact: artifact_id.clone(),
                function_selector,
            };
            if function_override.config.allow_internal_expect_revert == Some(true) {
                allow_internal_expect_revert.insert(identifier.clone());
            }
            test_function_overrides.insert(identifier, function_override.config.clone());
        }

        Self {
            test_function_overrides,
            allow_internal_expect_revert,
            eip712_types: source.eip712_types.clone(),
        }
    }
}
