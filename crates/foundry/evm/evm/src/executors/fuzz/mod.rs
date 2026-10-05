use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU32, Ordering},
        OnceLock,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use alloy_dyn_abi::JsonAbiExt;
use alloy_json_abi::Function;
use alloy_primitives::{keccak256, map::AddressHashMap, Address, Bytes, Log, U256};
use derive_where::derive_where;
use edr_decoder_revert::{cheatcodes::skip::SkipReason, RevertDecoder};
use foundry_evm_core::{
    constants::{CHEATCODE_ADDRESS, MAGIC_ASSUME},
    evm_context::{
        BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr,
        TransactionErrorTrait,
    },
};
use foundry_evm_coverage::HitMaps;
use foundry_evm_fuzz::{
    strategies::{fuzz_calldata, fuzz_calldata_from_state, EvmFuzzState},
    BaseCounterExample, CounterExample, FuzzCase, FuzzConfig, FuzzError, FuzzFixtures,
    FuzzRunMetadata, FuzzTestResult,
};
use foundry_evm_traces::SparsedTraceArena;
use proptest::{
    strategy::{Strategy, ValueTree},
    test_runner::{RngAlgorithm, TestCaseError, TestRng, TestRunner},
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use revm::context::result::{HaltReason, HaltReasonTr};

use crate::executors::{Executor, FuzzTestTimer};

mod types;
pub use types::{CaseOutcome, CounterExampleOutcome, FuzzOutcome};

use crate::executors::fuzz::types::CounterExampleData;

/// Minimum number of runs per worker.
/// This is mainly to reduce the overall number of rayon jobs.
const MIN_RUNS_PER_WORKER: u32 = 64;

/// Data collected by a single fuzz worker.
#[derive_where(Default; BlockT, HardforkT, TxT)]
struct WorkerState<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
> {
    /// Worker identifier.
    id: usize,
    /// First fuzz case this worker encountered (with global run number).
    first_case: Option<(u32, FuzzCase)>,
    /// Gas usage for all cases this worker ran.
    gas_by_case: Vec<(u64, u64)>,
    /// Counterexample if this worker found one.
    counterexample: CounterExampleData<
        BlockT,
        TxT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
    >,
    /// Traces collected by this worker.
    ///
    /// Stores up to `max_traces_to_collect` which is `config.gas_report_samples
    /// / num_workers`.
    traces: Vec<SparsedTraceArena>,
    /// Coverage collected by this worker.
    coverage: Option<HitMaps>,
    /// Logs of the failing call and, with `show_logs`, of all passing cases
    /// this worker ran.
    logs: Vec<Log>,
    /// Deprecated cheatcodes seen by this worker.
    deprecated_cheatcodes: HashMap<&'static str, Option<&'static str>>,
    /// Number of runs this worker completed.
    runs: u32,
    /// Failure reason if this worker failed.
    failure: Option<TestCaseError>,
    /// Fuzz run that produced the failure.
    failure_run: Option<FuzzRunMetadata>,
    /// Last run timestamp in milliseconds.
    ///
    /// Used to identify which worker ran last and collect its traces.
    last_run_timestamp: u128,
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
    >
    WorkerState<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>
{
    fn new(worker_id: usize) -> Self {
        Self {
            id: worker_id,
            ..Default::default()
        }
    }
}

/// Shared state for coordinating parallel fuzz workers.
struct SharedFuzzState {
    /// The fuzz dictionary, shared by all workers.
    state: EvmFuzzState,
    /// Total runs across workers.
    total_runs: AtomicU32,
    /// Found failure.
    ///
    /// The worker that found the failure sets its ID.
    ///
    /// This ID is then used to correctly extract the failure reason and
    /// counterexample.
    failed_worker_id: OnceLock<usize>,
    /// Total rejects across workers.
    total_rejects: AtomicU32,
    /// Fuzz timer.
    timer: FuzzTestTimer,
}

impl SharedFuzzState {
    fn new(state: EvmFuzzState, timeout: Option<u32>) -> Self {
        Self {
            state,
            total_runs: AtomicU32::new(0),
            failed_worker_id: OnceLock::new(),
            total_rejects: AtomicU32::new(0),
            timer: FuzzTestTimer::new(timeout),
        }
    }

    /// Increments the number of runs and returns the new value.
    fn increment_runs(&self) -> u32 {
        self.total_runs.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Increments and returns the new value of the number of rejected tests.
    fn increment_rejects(&self) -> u32 {
        self.total_rejects.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Returns `true` if the worker should continue running.
    fn should_continue(&self) -> bool {
        self.failed_worker_id.get().is_none() && !self.timer.is_timed_out()
    }

    /// Returns true if the worker was able to claim the failure, false if
    /// failure was set by another worker.
    fn try_claim_failure(&self, worker_id: usize) -> bool {
        let mut claimed = false;
        let _ = self.failed_worker_id.get_or_init(|| {
            claimed = true;
            worker_id
        });
        claimed
    }
}

/// Wrapper around an [`Executor`] which provides fuzzing support using
/// [`proptest`].
///
/// After instantiation, calling `fuzz` will proceed to hammer the deployed
/// smart contract with inputs, until it finds a counterexample. The runs are
/// split between parallel workers, each running its own clone of the executor.
/// The provided [`TestRunner`] contains all the configuration which can be
/// overridden via [environment variables](proptest::test_runner::Config)
pub struct FuzzedExecutor<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
> {
    /// The EVM executor.
    executor: Executor<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >,
    /// The fuzzer
    runner: TestRunner,
    /// The account that calls tests.
    sender: Address,
    /// The fuzz configuration.
    config: FuzzConfig,
    /// The persisted counterexample to be replayed, if any.
    persisted_failure: Option<BaseCounterExample>,
    /// The number of parallel workers.
    num_workers: usize,
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
    >
    FuzzedExecutor<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >
{
    /// Instantiates a fuzzed executor given a testrunner
    pub fn new(
        executor: Executor<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
        runner: TestRunner,
        sender: Address,
        config: FuzzConfig,
        persisted_failure: Option<BaseCounterExample>,
    ) -> Self {
        let num_workers = num_workers(config.runs, config.workers);
        Self {
            executor,
            runner,
            sender,
            config,
            persisted_failure,
            num_workers,
        }
    }

    /// Stores fuzz state for use with [`fuzz_calldata_from_state`]
    pub fn build_fuzz_state(&self, deployed_libs: &[Address]) -> EvmFuzzState {
        if let Some(fork_db) = self.executor.backend.active_fork_db() {
            EvmFuzzState::new(fork_db, self.config.dictionary, deployed_libs)
        } else {
            EvmFuzzState::new(
                self.executor.backend.mem_db(),
                self.config.dictionary,
                deployed_libs,
            )
        }
    }

    /// Whether tracing is on and if it records EVM step level data.
    pub fn tracer_records_steps(&self) -> bool {
        self.executor.tracer_records_steps()
    }

    /// The number of parallel workers the fuzz test is split between.
    pub fn num_workers(&self) -> usize {
        self.num_workers
    }
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: 'static
            + EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: 'static + HaltReasonTr + TryInto<HaltReason>,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: 'static + ChainContextTr + Send + Sync,
    >
    FuzzedExecutor<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >
{
    /// Fuzzes the provided function, assuming it is available at the contract
    /// at `address` If `should_fail` is set to `true`, then it will stop
    /// only when there's a success test case.
    ///
    /// The runs are distributed between `num_workers` workers on the global
    /// rayon pool. Each worker enters `tokio_handle`, so that fork-mode RPC
    /// calls made from its thread find a reactor.
    ///
    /// Returns a list of all the consumed gas and calldata of every fuzz case.
    pub fn fuzz(
        &self,
        func: &Function,
        fuzz_fixtures: &FuzzFixtures,
        deployed_libs: &[Address],
        address: Address,
        rd: &RevertDecoder,
        tokio_handle: &tokio::runtime::Handle,
    ) -> FuzzTestResult {
        let state = self.build_fuzz_state(deployed_libs);
        let shared_state = SharedFuzzState::new(state, self.config.timeout);

        // Replay the persisted counterexample before spawning workers, so that a
        // still failing counterexample is reported instead of whichever failure a
        // worker happens to find first.
        if self.num_workers > 0
            && let Some(replay) = self.replay_persisted_failure(func, address, rd, &shared_state)
        {
            let result = self.aggregate_results(vec![replay], func, &shared_state);
            shared_state.state.log_stats();
            return result;
        }

        debug!(n = self.num_workers, "spawning fuzz workers");
        let workers = (0..self.num_workers)
            .into_par_iter()
            .map(|worker_id| {
                let _tokio_guard = tokio_handle.enter();
                let _span_guard = info_span!("fuzz_worker", id = worker_id).entered();
                self.run_worker(worker_id, func, fuzz_fixtures, address, rd, &shared_state)
            })
            .collect::<Vec<_>>();

        let result = self.aggregate_results(workers, func, &shared_state);

        shared_state.state.log_stats();

        result
    }

    /// Replays the persisted counterexample, if any, against the fuzzed
    /// function.
    ///
    /// Returns the worker state holding the failure if the counterexample still
    /// fails. A counterexample recorded for a different function signature, one
    /// that passes, and one that is rejected by `vm.assume` are ignored.
    fn replay_persisted_failure(
        &self,
        func: &Function,
        address: Address,
        rd: &RevertDecoder,
        shared_state: &SharedFuzzState,
    ) -> Option<
        WorkerState<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    > {
        const WORKER_ID: usize = 0;

        let failure = self.persisted_failure.as_ref()?;
        if !failure
            .calldata
            .get(..4)
            .is_some_and(|selector| func.selector() == selector)
        {
            return None;
        }

        let mut worker = WorkerState::new(WORKER_ID);
        worker.last_run_timestamp = unix_timestamp_millis();

        // Reseed `vm.random*` as in the run that produced the counterexample.
        // Counterexamples persisted before the run was recorded fall back to
        // the first run of the first worker under the configured seed.
        let mut executor = self.executor.clone();
        let seed = failure.fuzz.seed.or(self.config.seed);
        let run = failure.fuzz.run.unwrap_or(1);
        let worker_id = failure.fuzz.worker.unwrap_or(WORKER_ID as u32);
        if let Some(cheats) = executor.inspector_mut().cheatcodes.as_mut()
            && let Some(seed) = seed
        {
            cheats.set_seed(fuzz_run_seed(seed, worker_id as usize, run));
        }
        let failure_run = Some(FuzzRunMetadata::new(seed, Some(run), Some(worker_id)));

        match self.single_fuzz(&executor, address, failure.calldata.clone()) {
            Ok(FuzzOutcome::Case(_)) | Err(TestCaseError::Reject(_)) => None,
            Ok(FuzzOutcome::CounterExample(CounterExampleOutcome {
                exit_reason: status,
                counterexample: outcome,
            })) => {
                shared_state.increment_runs();
                worker.runs += 1;

                let reason = rd.maybe_decode(&outcome.call.result, status);
                worker.logs.extend(outcome.call.logs.clone());
                worker.counterexample = outcome;
                // HACK: we have to use an empty string here to denote `None`.
                worker.failure = Some(TestCaseError::fail(reason.unwrap_or_default()));
                worker.failure_run = failure_run;
                shared_state.try_claim_failure(WORKER_ID);
                Some(worker)
            }
            Err(err @ TestCaseError::Fail(_)) => {
                worker.failure = Some(err);
                shared_state.try_claim_failure(WORKER_ID);
                Some(worker)
            }
        }
    }

    /// Runs a single fuzz worker.
    fn run_worker(
        &self,
        worker_id: usize,
        func: &Function,
        fuzz_fixtures: &FuzzFixtures,
        address: Address,
        rd: &RevertDecoder,
        shared_state: &SharedFuzzState,
    ) -> WorkerState<
        BlockT,
        TxT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
    > {
        let dictionary_weight = self.config.dictionary.dictionary_weight.min(100);
        let strategy = proptest::prop_oneof![
            100 - dictionary_weight => fuzz_calldata(func.clone(), fuzz_fixtures),
            dictionary_weight => fuzz_calldata_from_state(func.clone(), &shared_state.state),
        ];

        let mut executor = self.executor.clone();
        let mut worker = WorkerState::new(worker_id);
        // We want to collect at least one trace which will be displayed to user.
        let max_traces_to_collect =
            Ord::max(1, self.config.gas_report_samples / self.num_workers as u32) as usize;

        let worker_runs = runs_per_worker(self.config.runs, self.num_workers, worker_id);
        debug!(worker_runs);

        let mut runner_config = self.runner.config().clone();
        runner_config.cases = worker_runs;

        // For deterministic parallel fuzzing, derive a unique seed for each worker.
        let worker_seed = self
            .config
            .seed
            .map(|seed| fuzz_worker_seed(seed, worker_id));
        let mut runner = if let Some(worker_seed) = worker_seed {
            trace!(target: "forge::test", ?worker_seed, "deterministic seed for worker {worker_id}");
            let rng = TestRng::from_seed(RngAlgorithm::ChaCha, &worker_seed.to_be_bytes::<32>());
            TestRunner::new_with_rng(runner_config, rng)
        } else {
            TestRunner::new(runner_config)
        };

        // Number of inputs this worker generated, rejected ones included.
        let mut generated_inputs: u32 = 0;

        // Continue while:
        // 1. Global state allows (not timed out, no failure found)
        // 2. Worker hasn't reached its specific run limit
        'stop: while shared_state.should_continue() && worker.runs < worker_runs {
            // Reseed the cheatcode RNG (`vm.random*`) so that every run draws a
            // different, yet reproducible, sequence of values. The seed advances
            // with every generated input, not only with every accepted run, so
            // that a run rejected by `vm.assume` based on `vm.random*` output is
            // not retried with the very same values.
            if let Some(cheats) = executor.inspector_mut().cheatcodes.as_mut()
                && let Some(seed) = self.config.seed
            {
                cheats.set_seed(fuzz_run_seed(seed, worker_id, generated_inputs + 1));
            }

            let input = match strategy.new_tree(&mut runner) {
                Ok(tree) => {
                    generated_inputs += 1;
                    tree.current()
                }
                Err(err) => {
                    worker.failure = Some(TestCaseError::fail(format!(
                        "failed to generate fuzzed input in worker {worker_id}: {err}"
                    )));
                    shared_state.try_claim_failure(worker_id);
                    break 'stop;
                }
            };

            let inc_runs = |worker: &mut WorkerState<_, _, _, _, _, _, _>| {
                let total_runs = shared_state.increment_runs();
                debug_assert!(
                    shared_state.timer.is_enabled() || total_runs <= self.config.runs,
                    "worker runs were not distributed correctly"
                );
                worker.runs += 1;
                total_runs
            };

            worker.last_run_timestamp = unix_timestamp_millis();
            match self.single_fuzz(&executor, address, input) {
                Ok(fuzz_outcome) => match fuzz_outcome {
                    FuzzOutcome::Case(case) => {
                        let total_runs = inc_runs(&mut worker);

                        worker.gas_by_case.push((case.case.gas, case.case.stipend));

                        if worker.first_case.is_none() {
                            worker.first_case = Some((total_runs, case.case));
                        }

                        if let Some(call_traces) = case.call_trace_arena {
                            if worker.traces.len() == max_traces_to_collect {
                                worker.traces.pop();
                            }
                            worker.traces.push(call_traces);
                        }

                        if self.config.show_logs {
                            worker.logs.extend(case.logs);
                        }

                        HitMaps::merge_opt(&mut worker.coverage, case.coverage);
                        worker.deprecated_cheatcodes = case.deprecated_cheatcodes;
                    }
                    FuzzOutcome::CounterExample(CounterExampleOutcome {
                        exit_reason: status,
                        counterexample: outcome,
                    }) => {
                        inc_runs(&mut worker);

                        let reason = rd.maybe_decode(&outcome.call.result, status);
                        worker.logs.extend(outcome.call.logs.clone());
                        worker.counterexample = outcome;
                        // HACK: we have to use an empty string here to denote `None`.
                        worker.failure = Some(TestCaseError::fail(reason.unwrap_or_default()));
                        worker.failure_run = self.config.seed.map(|seed| {
                            FuzzRunMetadata::new(
                                Some(seed),
                                Some(generated_inputs),
                                Some(u32::try_from(worker_id).expect("worker count fits in u32")),
                            )
                        });
                        shared_state.try_claim_failure(worker_id);
                        break 'stop;
                    }
                },
                Err(err) => match err {
                    TestCaseError::Fail(_) => {
                        worker.failure = Some(err);
                        shared_state.try_claim_failure(worker_id);
                        break 'stop;
                    }
                    TestCaseError::Reject(_) => {
                        // Rejected inputs do not count as runs. Apply max rejects if
                        // configured.
                        let max = self.config.max_test_rejects;
                        let total = shared_state.increment_rejects();
                        if max > 0 && total > max {
                            worker.failure =
                                Some(TestCaseError::reject(FuzzError::TooManyRejects(max)));
                            shared_state.try_claim_failure(worker_id);
                            break 'stop;
                        }
                    }
                },
            }
        }

        worker
    }

    /// Aggregates the results from all workers.
    fn aggregate_results(
        &self,
        mut workers: Vec<
            WorkerState<
                BlockT,
                TxT,
                ChainContextT,
                EvmBuilderT,
                HaltReasonT,
                HardforkT,
                TransactionErrorT,
            >,
        >,
        func: &Function,
        shared_state: &SharedFuzzState,
    ) -> FuzzTestResult {
        let mut result = FuzzTestResult {
            first_case: FuzzCase::default(),
            gas_by_case: Vec::new(),
            success: true,
            skipped: false,
            reason: None,
            counterexample: None,
            logs: Vec::new(),
            labeled_addresses: AddressHashMap::default(),
            call_trace_arena: None,
            gas_report_traces: Vec::new(),
            line_coverage: None,
            deprecated_cheatcodes: HashMap::default(),
        };
        if workers.is_empty() {
            return result;
        }

        // The first case is the one with the lowest global run number.
        result.first_case = workers
            .iter()
            .filter_map(|worker| worker.first_case.as_ref())
            .min_by_key(|(run, _)| *run)
            .map(|(_, case)| case.clone())
            .unwrap_or_default();
        let last_run_worker_idx = workers
            .iter()
            .enumerate()
            .max_by_key(|(_, worker)| worker.last_run_timestamp)
            .map_or(0, |(idx, _)| idx);

        let failed_worker_id = shared_state.failed_worker_id.get().copied();
        if let Some(failed_worker_id) = failed_worker_id {
            result.success = false;

            let failed_worker = workers
                .iter_mut()
                .find(|worker| worker.id == failed_worker_id)
                .expect("the failed worker id belongs to one of the workers");

            let CounterExampleData { calldata, call } =
                std::mem::take(&mut failed_worker.counterexample);
            result.labeled_addresses = call.labels;
            // Nothing reads `BaseCounterExample::traces`, so the failing
            // arena can move into `FuzzTestResult::call_trace_arena` rather than
            // be cloned for both.
            result.call_trace_arena = call.call_trace_arena;

            match &failed_worker.failure {
                Some(TestCaseError::Fail(reason)) => {
                    let reason = reason.to_string();
                    result.reason = (!reason.is_empty()).then_some(reason);
                    let args = if let Some(data) = calldata.get(4..) {
                        func.abi_decode_input(data).unwrap_or_default()
                    } else {
                        vec![]
                    };

                    result.counterexample = Some(CounterExample::Single(
                        BaseCounterExample::from_fuzz_call(
                            calldata,
                            &args,
                            // Nothing consumes counterexample arenas; see above.
                            None,
                            call.indeterminism_reasons,
                        )
                        .with_fuzz_metadata(failed_worker.failure_run.unwrap_or_default()),
                    ));
                }
                Some(TestCaseError::Reject(reason)) => {
                    let reason = reason.to_string();
                    result.reason = (!reason.is_empty()).then_some(reason);
                }
                None => {}
            }
        } else {
            // The last trace of the worker that ran last is displayed to the user; the
            // remaining traces are gas report samples.
            result.call_trace_arena = workers
                .get_mut(last_run_worker_idx)
                .and_then(|worker| worker.traces.pop());
        }

        // Every worker keeps at least one trace so that one can be displayed, but
        // the gas report gets at most the configured number of samples (minus the
        // displayed trace on success).
        let max_gas_report_traces = if result.success {
            self.config.gas_report_samples.saturating_sub(1)
        } else {
            self.config.gas_report_samples
        } as usize;

        for worker in workers {
            result.gas_by_case.extend(worker.gas_by_case);
            // The logs of the failing call are always reported. Those of passing
            // runs, and of a failure another worker claimed first, only with
            // `show_logs`.
            if self.config.show_logs || Some(worker.id) == failed_worker_id {
                result.logs.extend(worker.logs);
            }
            result.gas_report_traces.extend(
                worker
                    .traces
                    .into_iter()
                    .map(|arena| arena.arena)
                    .take(max_gas_report_traces - result.gas_report_traces.len()),
            );
            HitMaps::merge_opt(&mut result.line_coverage, worker.coverage);
            result
                .deprecated_cheatcodes
                .extend(worker.deprecated_cheatcodes);
        }

        if let Some(reason) = &result.reason
            && let Some(reason) = SkipReason::decode_self(reason)
        {
            result.skipped = true;
            result.reason = reason.0;
        }

        result
    }

    /// Granular and single-step function that runs only one fuzz and returns
    /// either a `CaseOutcome` or a `CounterExampleOutcome`
    #[allow(clippy::type_complexity)]
    fn single_fuzz(
        &self,
        executor: &Executor<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
        address: Address,
        calldata: Bytes,
    ) -> Result<
        FuzzOutcome<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
        TestCaseError,
    > {
        let mut call = executor
            .call_raw(self.sender, address, calldata.clone(), U256::ZERO)
            // Alternate formatting includes the error's chain: `to_string`
            // yields only the outermost layer — a bare "EVM error".
            .map_err(|e| TestCaseError::fail(format!("{e:#}")))?;

        // Handle `vm.assume`.
        if call.result.as_ref() == MAGIC_ASSUME {
            return Err(TestCaseError::reject(FuzzError::AssumeReject));
        }

        let deprecated_cheatcodes = call
            .cheatcodes
            .as_ref()
            .map_or_else(Default::default, |cheatcodes| {
                cheatcodes.deprecated.clone().into_iter().collect()
            });

        // Consider call success if test should not fail on reverts and reverter is not
        // the cheatcode or test address.
        let success = if !self.config.fail_on_revert
            && call
                .reverter
                .is_some_and(|reverter| reverter != address && reverter != CHEATCODE_ADDRESS)
        {
            true
        } else {
            executor.is_raw_call_mut_success(&mut call, false)
        };

        if success {
            Ok(FuzzOutcome::Case(CaseOutcome {
                case: FuzzCase {
                    calldata,
                    gas: call.gas_used,
                    stipend: call.stipend,
                },
                call_trace_arena: call.call_trace_arena,
                coverage: call.line_coverage,
                logs: call.logs,
                deprecated_cheatcodes,
            }))
        } else {
            Ok(FuzzOutcome::CounterExample(CounterExampleOutcome {
                exit_reason: call.exit_reason,
                counterexample: CounterExampleData { calldata, call },
            }))
        }
    }
}

/// Milliseconds since the UNIX epoch; `0` if the clock is before the epoch.
fn unix_timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis())
}

/// Determines the number of workers a fuzz test with `runs` runs is split
/// between.
///
/// Every worker gets at least [`MIN_RUNS_PER_WORKER`] runs. `workers` caps the
/// number of workers if set, otherwise the number of threads of the global
/// rayon pool does. A test with no runs gets no workers.
fn num_workers(runs: u32, workers: Option<u32>) -> usize {
    if runs == 0 {
        return 0;
    }
    let max_workers = Ord::max(1, runs / MIN_RUNS_PER_WORKER) as usize;
    match workers {
        Some(workers) => Ord::min(Ord::max(1, workers as usize), max_workers),
        None => Ord::min(rayon::current_num_threads(), max_workers),
    }
}

/// Determines the number of runs of worker `worker_id` out of `num_workers`
/// workers sharing `total_runs` runs.
fn runs_per_worker(total_runs: u32, num_workers: usize, worker_id: usize) -> u32 {
    let n = num_workers as u32;
    let runs = total_runs / n;
    let remainder = total_runs % n;
    // Distribute the remainder evenly among the first `remainder` workers,
    // assuming `worker_id` is in `0..n`.
    if (worker_id as u32) < remainder {
        runs + 1
    } else {
        runs
    }
}

/// Derives the RNG seed of worker `worker_id` from the configured fuzz seed.
///
/// The first worker uses the seed as is, so that single-worker runs reproduce
/// the inputs of a sequential run. Other workers use
/// `keccak256(seed || worker_id as u32 BE)`.
fn fuzz_worker_seed(seed: U256, worker_id: usize) -> U256 {
    if worker_id == 0 {
        return seed;
    }
    let worker_id = u32::try_from(worker_id).expect("worker count fits in u32");
    let seed_data = [&seed.to_be_bytes::<32>()[..], &worker_id.to_be_bytes()[..]].concat();
    U256::from_be_bytes(keccak256(seed_data).0)
}

/// Derives the cheatcode RNG seed of the `run`-th (1-based) input generated by
/// a worker, so that `vm.random*` draws the same values when the input is
/// replayed.
fn fuzz_run_seed(seed: U256, worker_id: usize, run: u32) -> U256 {
    fuzz_worker_seed(seed, worker_id).wrapping_add(U256::from(run.saturating_sub(1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_are_distributed_between_workers() {
        for (total_runs, num_workers) in [(256, 4), (257, 4), (1000, 3), (64, 1), (5, 2)] {
            let runs: Vec<u32> = (0..num_workers)
                .map(|worker_id| runs_per_worker(total_runs, num_workers, worker_id))
                .collect();
            assert_eq!(
                runs.iter().sum::<u32>(),
                total_runs,
                "{total_runs} runs / {num_workers} workers"
            );
            let (min, max) = (runs.iter().min().unwrap(), runs.iter().max().unwrap());
            assert!(max - min <= 1, "{runs:?}");
        }
    }

    #[test]
    fn worker_count_is_bounded_by_runs() {
        assert_eq!(num_workers(0, None), 0);
        assert_eq!(num_workers(0, Some(4)), 0);
        assert_eq!(num_workers(1, None), 1);
        assert_eq!(num_workers(63, Some(8)), 1);
        assert_eq!(num_workers(256, Some(8)), 4);
        assert_eq!(num_workers(256, Some(0)), 1);
        assert_eq!(num_workers(10_000, Some(3)), 3);
        assert!(num_workers(10_000, None) <= rayon::current_num_threads());
    }

    #[test]
    fn worker_seeds_are_deterministic_and_distinct() {
        let seed = U256::from(997u32);
        assert_eq!(fuzz_worker_seed(seed, 0), seed);
        let seeds: Vec<U256> = (0..8).map(|id| fuzz_worker_seed(seed, id)).collect();
        for (i, a) in seeds.iter().enumerate() {
            assert_eq!(*a, fuzz_worker_seed(seed, i));
            for b in &seeds[i + 1..] {
                assert_ne!(a, b);
            }
        }
        assert_ne!(
            fuzz_worker_seed(seed, 1),
            fuzz_worker_seed(U256::from(998u32), 1)
        );
    }
}
