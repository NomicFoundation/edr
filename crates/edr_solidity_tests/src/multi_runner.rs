//! Forge test runner for multiple contracts.

use std::{collections::BTreeMap, fmt::Debug, path::PathBuf, sync::Arc, time::Instant};

use alloy_json_abi::JsonAbi;
use alloy_primitives::Bytes;
use edr_solidity::{artifacts::ArtifactId, contract_decoder::SyncNestedTraceDecoder};
use eyre::Result;
use foundry_evm::{
    contracts::ContractsByArtifact,
    decode::RevertDecoder,
    executors::ExecutorBuilder,
    fork::CreateFork,
    inspectors::{cheatcodes::CheatsConfigOptions, CheatsConfig},
    opts::EvmOpts,
    revm,
    traces::{decode_trace_arena, identifier::TraceIdentifiers, CallTraceDecoderBuilder},
};
use futures::StreamExt;

use crate::{
    result::SuiteResult,
    runner::{ContractRunnerArtifacts, ContractRunnerOptions},
    ContractRunner, ShowTraces, SolidityTestRunnerConfig, SolidityTestRunnerConfigError,
    TestFilter, TestOptions,
};

/// A deployable test contract
#[derive(Debug, Clone)]
pub struct TestContract {
    /// The test contract abi
    pub abi: JsonAbi,
    /// The test contract bytecode
    pub bytecode: Bytes,
}

pub type TestContracts = BTreeMap<ArtifactId, TestContract>;

/// A multi contract runner receives a set of contracts deployed in an EVM
/// instance and proceeds to run all test functions in these contracts.
#[derive(Clone, Debug)]
pub struct MultiContractRunner<NestedTraceDecoderT> {
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
    evm_opts: EvmOpts,
    /// The configured evm
    env: revm::primitives::Env,
    /// Revert decoder. Contains all known errors and their selectors.
    revert_decoder: RevertDecoder,
    /// The fork to use at launch
    fork: Option<CreateFork>,
    /// Whether to collect coverage info
    coverage: bool,
    /// Whether to enable trace mode and which traces to include in test
    /// results.
    traces: ShowTraces,
    /// Whether to support the `testFail` prefix
    test_fail: bool,
    /// Whether to enable solidity fuzz fixtures support
    solidity_fuzz_fixtures: bool,
    /// Settings related to fuzz and/or invariant tests
    test_options: TestOptions,
}

impl<NestedTraceDecoderT: SyncNestedTraceDecoder> MultiContractRunner<NestedTraceDecoderT> {
    /// Creates a new multi contract runner.
    pub async fn new(
        config: SolidityTestRunnerConfig,
        test_contracts: TestContracts,
        known_contracts: ContractsByArtifact,
        libs_to_deploy: Vec<Bytes>,
        contract_decoder: NestedTraceDecoderT,
        revert_decoder: RevertDecoder,
    ) -> Result<MultiContractRunner<NestedTraceDecoderT>, SolidityTestRunnerConfigError> {
        let env = config
            .evm_opts
            .evm_env()
            .await
            .map_err(SolidityTestRunnerConfigError::EvmEnv)?;

        let fork = config.get_fork().await?;

        let SolidityTestRunnerConfig {
            traces,
            coverage,
            test_fail,
            evm_opts,
            project_root,
            cheats_config_options,
            fuzz,
            invariant,
            solidity_fuzz_fixtures,
        } = config;

        // Do canonicalization in blocking context.
        // Canonicalization can touch the file system, hence the blocking thread
        let project_root = tokio::task::spawn_blocking(move || {
            dunce::canonicalize(project_root)
                .map_err(SolidityTestRunnerConfigError::InvalidProjectRoot)
        })
        .await
        .expect("Thread shouldn't panic")?;

        let test_options: TestOptions = TestOptions { fuzz, invariant };

        Ok(Self {
            project_root,
            test_contracts,
            known_contracts: Arc::new(known_contracts),
            contract_decoder: Arc::new(contract_decoder),
            libs_to_deploy,
            cheats_config_options: Arc::new(cheats_config_options),
            evm_opts,
            env,
            revert_decoder,
            fork,
            coverage,
            traces,
            test_fail,
            solidity_fuzz_fixtures,
            test_options,
        })
    }

    /// Returns the known contracts.
    pub fn known_contracts(&self) -> &ContractsByArtifact {
        &self.known_contracts
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
    ) -> BTreeMap<String, SuiteResult> {
        let (tx_results, mut rx_results) =
            tokio::sync::mpsc::unbounded_channel::<(ArtifactId, SuiteResult)>();

        self.test(Arc::new(filter), tx_results);

        let mut results = BTreeMap::new();

        while let Some((id, result)) = rx_results.recv().await {
            results.insert(id.identifier(), result);
        }

        results
    }

    /// Executes _all_ tests that match the given `filter`.
    ///
    /// The method will immediately return and send the results to the given
    /// channel as they're ready.
    ///
    /// This will create the runtime based on the configured `evm` ops and
    /// create the `Backend` before executing all contracts and their tests
    /// in _parallel_.
    ///
    /// Each Executor gets its own instance of the `Backend`.
    pub fn test(
        mut self,
        filter: Arc<impl TestFilter + 'static>,
        tx: tokio::sync::mpsc::UnboundedSender<(ArtifactId, SuiteResult)>,
    ) {
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

        let handle = tokio::runtime::Handle::current();

        let this = Arc::new(self);
        let args = contracts
            .into_iter()
            .zip(std::iter::repeat((this, fork, filter, tx)));

        handle.spawn(async {
            futures::stream::iter(args)
                .for_each_concurrent(
                    Some(num_cpus::get()),
                    |((id, contract), (this, fork, filter, tx))| async move {
                        tokio::task::spawn_blocking(move || {
                            let handle = tokio::runtime::Handle::current();
                            let result =
                                this.run_tests(&id, &contract, fork, filter.as_ref(), &handle);
                            let _ = tx.send((id, result));
                        })
                        .await
                        .expect("failed to join task");
                    },
                )
                .await;
        });
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

    fn run_tests(
        &self,
        artifact_id: &ArtifactId,
        contract: &TestContract,
        fork: Option<CreateFork>,
        filter: &dyn TestFilter,
        handle: &tokio::runtime::Handle,
    ) -> SuiteResult {
        let identifier = artifact_id.identifier();
        let mut span_name = identifier.as_str();

        let cheats_config = CheatsConfig::new(
            self.project_root.clone(),
            (*self.cheats_config_options).clone(),
            self.evm_opts.clone(),
            self.known_contracts.clone(),
            Some(artifact_id.version.clone()),
        );

        let executor_builder = ExecutorBuilder::new()
            .env(self.env.clone())
            .fork(fork)
            .gas_limit(self.evm_opts.gas_limit())
            .inspectors(|stack| {
                stack
                    .cheatcodes(Arc::new(cheats_config))
                    .trace(self.traces != ShowTraces::None)
                    .coverage(self.coverage)
                    .enable_isolation(self.evm_opts.isolate)
            })
            .spec(self.evm_opts.spec);

        if !enabled!(tracing::Level::TRACE) {
            span_name = &artifact_id.name;
        }
        let _guard = info_span!("run_tests", name = span_name).entered();

        debug!("start executing all tests in contract");

        let runner = ContractRunner::new(
            &identifier,
            executor_builder,
            contract,
            ContractRunnerArtifacts {
                revert_decoder: &self.revert_decoder,
                known_contracts: &self.known_contracts,
                libs_to_deploy: &self.libs_to_deploy,
                contract_decoder: Arc::clone(&self.contract_decoder),
            },
            ContractRunnerOptions {
                initial_balance: self.evm_opts.initial_balance,
                sender: self.evm_opts.sender,
                test_fail: self.test_fail,
                solidity_fuzz_fixtures: self.solidity_fuzz_fixtures,
            },
        );
        let mut r = runner.run_tests(filter, &self.test_options, handle);

        if self.traces != ShowTraces::None {
            let mut decoder = CallTraceDecoderBuilder::new().build();

            for (_, result) in &mut r.test_results {
                decoder.clear_addresses();
                decoder.labels.extend(
                    result
                        .labeled_addresses
                        .iter()
                        .map(|(k, v)| (*k, v.clone())),
                );

                for (_, arena) in &mut result.traces {
                    let mut trace_identifier =
                        TraceIdentifiers::new().with_local(&self.known_contracts);
                    decoder.identify(&arena, &mut trace_identifier);
                    tokio::task::block_in_place(|| {
                        handle.block_on(decode_trace_arena(arena, &mut decoder))
                    });
                }
            }
        }

        debug!(duration=?r.duration, "executed all tests in contract");

        r
    }
}

fn matches_contract(id: &ArtifactId, filter: &dyn TestFilter) -> bool {
    filter.matches_path(&id.source) && filter.matches_contract(&id.name)
}
