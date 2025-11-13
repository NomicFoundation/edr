//! Test outcomes.

use std::{
    collections::{BTreeMap, HashMap},
    fmt::{self, Write},
    time::Duration,
};

use alloy_primitives::{map::AddressHashMap, Address, Log};
use derive_where::derive_where;
use edr_chain_spec::HaltReasonTrait;
pub use foundry_evm::executors::invariant::InvariantMetrics;
use foundry_evm::{
    coverage::HitMaps,
    evm_context::{
        BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr,
        TransactionErrorTrait,
    },
    executors::{invariant::InvariantFuzzError, stack_trace::StackTraceResult, RawCallResult},
    fuzz::{CounterExample, FuzzFixtures},
    traces::{CallTraceArena, CallTraceDecoder, TraceKind, Traces},
};
use serde::{Deserialize, Serialize};
use yansi::Paint;

use crate::{
    backend::IndeterminismReasons,
    decode::{decode_console_logs, SkipReason},
    fuzz::{BaseCounterExample, FuzzTestResult, FuzzedCases},
    gas_report::GasReport,
};

/// The aggregated result of a test run.
#[derive(Clone, Debug)]
pub struct TestOutcome<HaltReasonT: HaltReasonTrait> {
    /// The results of all test suites by their identifier
    /// (`path:contract_name`).
    ///
    /// Essentially `identifier => signature => result`.
    pub results: BTreeMap<String, SuiteResult<HaltReasonT>>,
    /// Whether to allow test failures without failing the entire test run.
    pub allow_failure: bool,
    /// The decoder used to decode traces and logs.
    ///
    /// This is `None` if traces and logs were not decoded.
    ///
    /// Note that `Address` fields only contain the last executed test case's
    /// data.
    pub last_run_decoder: Option<CallTraceDecoder>,
    /// The gas report, if requested.
    pub gas_report: Option<GasReport>,
}

impl<HaltReasonT: HaltReasonTrait> TestOutcome<HaltReasonT> {
    /// Creates a new test outcome with the given results.
    pub fn new(results: BTreeMap<String, SuiteResult<HaltReasonT>>, allow_failure: bool) -> Self {
        Self {
            results,
            allow_failure,
            last_run_decoder: None,
            gas_report: None,
        }
    }

    /// Creates a new empty test outcome.
    pub fn empty(allow_failure: bool) -> Self {
        Self::new(BTreeMap::new(), allow_failure)
    }

    /// Returns an iterator over all individual succeeding tests and their
    /// names.
    pub fn successes(&self) -> impl Iterator<Item = (&String, &TestResult<HaltReasonT>)> {
        self.tests()
            .filter(|(_, t)| t.status == TestStatus::Success)
    }

    /// Returns an iterator over all individual skipped tests and their names.
    pub fn skips(&self) -> impl Iterator<Item = (&String, &TestResult<HaltReasonT>)> {
        self.tests()
            .filter(|(_, t)| t.status == TestStatus::Skipped)
    }

    /// Returns an iterator over all individual failing tests and their names.
    pub fn failures(&self) -> impl Iterator<Item = (&String, &TestResult<HaltReasonT>)> {
        self.tests()
            .filter(|(_, t)| t.status == TestStatus::Failure)
    }

    /// Returns an iterator over all individual tests and their names.
    pub fn tests(&self) -> impl Iterator<Item = (&String, &TestResult<HaltReasonT>)> {
        self.results.values().flat_map(SuiteResult::tests)
    }

    /// Flattens the test outcome into a list of individual tests.
    pub fn into_tests(self) -> impl Iterator<Item = SuiteTestResult<HaltReasonT>> {
        self.results
            .into_iter()
            .flat_map(|(file, suite)| {
                suite
                    .test_results
                    .into_iter()
                    .map(move |t| (file.clone(), t))
            })
            .map(|(artifact_id, (signature, result))| SuiteTestResult {
                artifact_id,
                signature,
                result,
            })
    }

    /// Returns the number of tests that passed.
    pub fn passed(&self) -> usize {
        self.successes().count()
    }

    /// Returns the number of tests that were skipped.
    pub fn skipped(&self) -> usize {
        self.skips().count()
    }

    /// Returns the number of tests that failed.
    pub fn failed(&self) -> usize {
        self.failures().count()
    }

    /// Sums up all the durations of all individual test suites.
    ///
    /// Note that this is not necessarily the wall clock time of the entire test
    /// run.
    pub fn total_time(&self) -> Duration {
        self.results.values().map(|suite| suite.duration).sum()
    }

    /// Formats the aggregated summary of all test suites into a string (for
    /// printing).
    pub fn summary(&self, wall_clock_time: Duration) -> String {
        let num_test_suites = self.results.len();
        let suites = if num_test_suites == 1 {
            "suite"
        } else {
            "suites"
        };
        let total_passed = self.passed();
        let total_failed = self.failed();
        let total_skipped = self.skipped();
        let total_tests = total_passed + total_failed + total_skipped;
        format!(
            "\nRan {} test {} in {:.2?} ({:.2?} CPU time): {} tests passed, {} failed, {} skipped ({} total tests)",
            num_test_suites,
            suites,
            wall_clock_time,
            self.total_time(),
            total_passed.green(),
            total_failed.red(),
            total_skipped.yellow(),
            total_tests
        )
    }
}

/// A set of test results for a single test suite, which is all the tests in a
/// single contract.
#[derive(Clone, Debug, Serialize)]
pub struct SuiteResult<HaltReasonT> {
    /// Wall clock time it took to execute all tests in this suite.
    #[serde(with = "humantime_serde")]
    pub duration: Duration,
    /// Individual test results: `test fn signature -> TestResult`.
    pub test_results: BTreeMap<String, TestResult<HaltReasonT>>,
    /// Generated warnings.
    pub warnings: Vec<String>,
}

impl<HaltReasonT> SuiteResult<HaltReasonT>
where
    HaltReasonT: HaltReasonTrait,
{
    pub fn map_halt_reason<
        ConversionFnT: Copy + Fn(HaltReasonT) -> NewHaltReasonT,
        NewHaltReasonT,
    >(
        self,
        conversion_fn: ConversionFnT,
    ) -> SuiteResult<NewHaltReasonT> {
        let test_results = self
            .test_results
            .into_iter()
            .map(|(name, result)| (name, result.map_halt_reason(conversion_fn)))
            .collect();
        SuiteResult {
            duration: self.duration,
            test_results,
            warnings: self.warnings,
        }
    }
}

impl<HaltReasonT: HaltReasonTrait> SuiteResult<HaltReasonT> {
    pub fn new(
        duration: Duration,
        test_results: BTreeMap<String, TestResult<HaltReasonT>>,
        mut warnings: Vec<String>,
    ) -> Self {
        // Add deprecated cheatcodes warning, if any of them used in current test suite.
        let mut deprecated_cheatcodes = HashMap::new();

        for test_result in test_results.values() {
            deprecated_cheatcodes.extend(&test_result.deprecated_cheatcodes);
        }

        if !deprecated_cheatcodes.is_empty() {
            let mut warning =
                "The following cheatcode(s) are deprecated and will be removed in future versions:"
                    .to_owned();

            for (cheatcode, reason) in deprecated_cheatcodes {
                write!(warning, "\n  {cheatcode}").unwrap();

                if let Some(reason) = reason {
                    write!(warning, ": {reason}").unwrap();
                }
            }

            warnings.push(warning);
        }

        Self {
            duration,
            test_results,
            warnings,
        }
    }

    /// Returns an iterator over all individual succeeding tests and their
    /// names.
    pub fn successes(&self) -> impl Iterator<Item = (&String, &TestResult<HaltReasonT>)> {
        self.tests()
            .filter(|(_, t)| t.status == TestStatus::Success)
    }

    /// Returns an iterator over all individual skipped tests and their names.
    pub fn skips(&self) -> impl Iterator<Item = (&String, &TestResult<HaltReasonT>)> {
        self.tests()
            .filter(|(_, t)| t.status == TestStatus::Skipped)
    }

    /// Returns an iterator over all individual failing tests and their names.
    pub fn failures(&self) -> impl Iterator<Item = (&String, &TestResult<HaltReasonT>)> {
        self.tests()
            .filter(|(_, t)| t.status == TestStatus::Failure)
    }

    /// Returns the number of tests that passed.
    pub fn passed(&self) -> usize {
        self.successes().count()
    }

    /// Returns the number of tests that were skipped.
    pub fn skipped(&self) -> usize {
        self.skips().count()
    }

    /// Returns the number of tests that failed.
    pub fn failed(&self) -> usize {
        self.failures().count()
    }

    /// Iterator over all tests and their names
    pub fn tests(&self) -> impl Iterator<Item = (&String, &TestResult<HaltReasonT>)> {
        self.test_results.iter()
    }

    /// Whether this test suite is empty.
    pub fn is_empty(&self) -> bool {
        self.test_results.is_empty()
    }

    /// The number of tests in this test suite.
    pub fn len(&self) -> usize {
        self.test_results.len()
    }

    /// Sums up all the durations of all individual tests in this suite.
    ///
    /// Note that this is not necessarily the wall clock time of the entire test
    /// suite.
    pub fn total_time(&self) -> Duration {
        self.test_results
            .values()
            .map(|result| result.duration)
            .sum()
    }

    /// Returns the summary of a single test suite.
    pub fn summary(&self) -> String {
        let failed = self.failed();
        let result = if failed == 0 {
            "ok".green()
        } else {
            "FAILED".red()
        };
        format!(
            "Suite result: {}. {} passed; {} failed; {} skipped; finished in {:.2?} ({:.2?} CPU time)",
            result,
            self.passed().green(),
            failed.red(),
            self.skipped().yellow(),
            self.duration,
            self.total_time(),
        )
    }
}

/// The result of a single test in a test suite.
///
/// This is flattened from a [`TestOutcome`].
#[derive(Clone, Debug)]
pub struct SuiteTestResult<HaltReasonT: HaltReasonTrait> {
    /// The identifier of the artifact/contract in the form:
    /// `<artifact file name>:<contract name>`.
    pub artifact_id: String,
    /// The function signature of the Solidity test.
    pub signature: String,
    /// The result of the executed test.
    pub result: TestResult<HaltReasonT>,
}

/// The status of a test.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestStatus {
    Success,
    #[default]
    Failure,
    Skipped,
}

impl TestStatus {
    /// Returns `true` if the test was successful.
    #[inline]
    pub fn is_success(self) -> bool {
        matches!(self, Self::Success)
    }

    /// Returns `true` if the test failed.
    #[inline]
    pub fn is_failure(self) -> bool {
        matches!(self, Self::Failure)
    }

    /// Returns `true` if the test was skipped.
    #[inline]
    pub fn is_skipped(self) -> bool {
        matches!(self, Self::Skipped)
    }
}

/// The result of an executed test.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[derive_where(Default)]
pub struct TestResult<HaltReasonT> {
    /// The test status, indicating whether the test case succeeded, failed, or
    /// was marked as skipped. This means that the transaction executed
    /// properly, the test was marked as skipped with `vm.skip()`, or that
    /// there was a revert and that the test was expected to fail (prefixed
    /// with `testFail`)
    pub status: TestStatus,

    /// If there was a revert, this field will be populated. Note that the test
    /// can still be successful (i.e self.success == true) when it's
    /// expected to fail.
    pub reason: Option<String>,

    /// Minimal reproduction test case for failing test
    pub counterexample: Option<CounterExample>,

    /// Any captured & parsed as strings logs along the test's execution which
    /// should be printed to the user.
    pub logs: Vec<Log>,

    /// The decoded `DSTest` logging events and Hardhat's `console.log` from
    /// [logs](Self::logs).
    pub decoded_logs: Vec<String>,

    /// What kind of test this was
    pub kind: TestKind,

    /// Traces
    #[serde(skip)]
    pub traces: Traces,

    /// Additional traces to use for gas report.
    #[serde(skip)]
    pub gas_report_traces: Vec<Vec<CallTraceArena>>,

    /// Raw coverage info
    #[serde(skip)]
    pub line_coverage: Option<HitMaps>,

    /// Labeled addresses
    pub labeled_addresses: AddressHashMap<String>,

    /// Wall clock execution time.
    pub duration: Duration,

    /// Any captured value snapshots (incl. gas) along the test's
    /// execution which should be accumulated.
    pub value_snapshots: BTreeMap<String, BTreeMap<String, String>>,

    /// The outcome of the stack trace error computation.
    /// None if the test status is succeeded or skipped.
    /// If the heuristic failed the vec is set but emtpy.
    /// Error if there was an error computing the stack trace.
    #[serde(skip)]
    pub stack_trace_result: Option<StackTraceResult<HaltReasonT>>,

    /// Deprecated cheatcodes (mapped to their replacements, if any) used in
    /// current test.
    #[serde(skip)]
    pub deprecated_cheatcodes: HashMap<&'static str, Option<&'static str>>,
}

impl<HaltReasonT> TestResult<HaltReasonT> {
    pub fn map_halt_reason<
        ConversionFnT: Copy + Fn(HaltReasonT) -> NewHaltReasonT,
        NewHaltReasonT,
    >(
        self,
        conversion_fn: ConversionFnT,
    ) -> TestResult<NewHaltReasonT> {
        TestResult {
            status: self.status,
            reason: self.reason,
            counterexample: self.counterexample,
            logs: self.logs,
            decoded_logs: self.decoded_logs,
            kind: self.kind,
            traces: self.traces,
            gas_report_traces: self.gas_report_traces,
            line_coverage: self.line_coverage,
            labeled_addresses: self.labeled_addresses,
            duration: self.duration,
            value_snapshots: self.value_snapshots,
            stack_trace_result: self
                .stack_trace_result
                .map(|s| s.map_halt_reason(conversion_fn)),
            deprecated_cheatcodes: self.deprecated_cheatcodes,
        }
    }
}

impl<HaltReasonT: HaltReasonTrait> fmt::Display for TestResult<HaltReasonT> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.status {
            TestStatus::Success => "[PASS]".green().fmt(f),
            TestStatus::Skipped => "[SKIP]".yellow().fmt(f),
            TestStatus::Failure => {
                let mut s = String::from("[FAIL. Reason: ");

                let reason = self.reason.as_deref().unwrap_or("assertion failed");
                s.push_str(reason);

                if let Some(counterexample) = &self.counterexample {
                    match counterexample {
                        CounterExample::Single(ex) => {
                            write!(s, "; counterexample: {ex}]").unwrap();
                        }
                        CounterExample::Sequence(original, sequence) => {
                            s.push_str(
                                format!(
                                    "]\n\t[Sequence] (original: {original}, shrunk: {})\n",
                                    sequence.len()
                                )
                                .as_str(),
                            );
                            for ex in sequence {
                                writeln!(s, "{ex}").unwrap();
                            }
                        }
                    }
                } else {
                    s.push(']');
                }

                s.red().fmt(f)
            }
        }
    }
}

impl<HaltReasonT: HaltReasonTrait> TestResult<HaltReasonT> {
    /// Creates a new test result starting from test setup results.
    pub fn new(setup: &TestSetup<HaltReasonT>) -> Self {
        Self {
            labeled_addresses: setup.labels.clone(),
            logs: setup.logs.clone(),
            decoded_logs: decode_console_logs(&setup.logs),
            traces: setup.traces.clone(),
            line_coverage: setup.coverage.clone(),
            stack_trace_result: setup.stack_trace_result.clone(),
            ..Default::default()
        }
    }

    /// Creates a failed test result with given reason.
    pub fn fail(reason: String) -> Self {
        Self {
            status: TestStatus::Failure,
            reason: Some(reason),
            ..Default::default()
        }
    }

    /// Creates a test setup result.
    pub fn setup_result(setup: TestSetup<HaltReasonT>) -> Self {
        Self {
            status: if setup.skipped {
                TestStatus::Skipped
            } else {
                TestStatus::Failure
            },
            reason: setup.reason,
            decoded_logs: decode_console_logs(&setup.logs),
            logs: setup.logs,
            traces: setup.traces,
            line_coverage: setup.coverage,
            labeled_addresses: setup.labels,
            stack_trace_result: setup.stack_trace_result,
            ..Default::default()
        }
    }

    /// Returns the skipped result for single test (used in skipped fuzz test
    /// too).
    pub fn single_skip(&mut self, reason: SkipReason) {
        self.status = TestStatus::Skipped;
        self.reason = reason.0;
    }

    /// Returns the failed result with reason for single test.
    pub fn single_fail(&mut self, reason: Option<String>, duration: Duration) {
        self.status = TestStatus::Failure;
        self.reason = reason;
        self.duration = duration;
    }

    /// Returns the result for single test. Merges execution results (logs,
    /// labeled addresses, traces and coverages) in initial setup results.
    pub fn single_result<
        BlockT: BlockEnvTr,
        ChainContextT: 'static + ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        TxT: TransactionEnvTr,
    >(
        &mut self,
        success: bool,
        reason: Option<String>,
        raw_call_result: RawCallResult<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
        duration: Duration,
    ) {
        self.kind = TestKind::Unit {
            gas: raw_call_result
                .gas_used
                .wrapping_sub(raw_call_result.stipend),
        };

        // Record logs, labels, traces and merge coverages.
        self.logs.extend(raw_call_result.logs);
        self.decoded_logs = decode_console_logs(&self.logs);
        self.labeled_addresses.extend(raw_call_result.labels);
        self.traces.extend(
            raw_call_result
                .traces
                .map(|traces| (TraceKind::Execution, traces)),
        );
        self.merge_coverages(raw_call_result.line_coverage);

        self.status = match success {
            true => TestStatus::Success,
            false => TestStatus::Failure,
        };
        self.reason = reason;
        self.duration = duration;
        self.gas_report_traces = Vec::new();

        if let Some(cheatcodes) = raw_call_result.cheatcodes {
            self.value_snapshots = cheatcodes.gas_snapshots;
            self.deprecated_cheatcodes = cheatcodes.deprecated.into_iter().collect();
        }
    }

    /// Returns the result for a fuzzed test. Merges fuzz execution results
    /// (logs, labeled addresses, traces and coverages) in initial setup
    /// results.
    pub fn fuzz_result(&mut self, result: FuzzTestResult, duration: Duration) {
        self.kind = TestKind::Fuzz {
            median_gas: result.median_gas(false),
            mean_gas: result.mean_gas(false),
            runs: result.gas_by_case.len(),
        };

        // Record logs, labels, traces and merge coverages.
        self.logs.extend(result.logs);
        self.decoded_logs = decode_console_logs(&self.logs);
        self.labeled_addresses.extend(result.labeled_addresses);
        self.traces
            .extend(result.traces.map(|traces| (TraceKind::Execution, traces)));
        self.merge_coverages(result.line_coverage);

        self.status = if result.skipped {
            TestStatus::Skipped
        } else if result.success {
            TestStatus::Success
        } else {
            TestStatus::Failure
        };
        self.reason = result.reason;
        self.counterexample = result.counterexample;
        self.duration = duration;
        self.gas_report_traces = result
            .gas_report_traces
            .into_iter()
            .map(|t| vec![t])
            .collect();
        self.deprecated_cheatcodes = result.deprecated_cheatcodes;
    }

    /// Returns the skipped result for invariant test.
    pub fn invariant_skip(&mut self, reason: SkipReason, duration: Duration) {
        self.kind = TestKind::Invariant {
            runs: 1,
            calls: 1,
            reverts: 1,
            metrics: HashMap::default(),
            failed_corpus_replays: 0,
        };
        self.status = TestStatus::Skipped;
        self.reason = reason.0;
        self.duration = duration;
    }

    /// Returns the fail result for replayed invariant test.
    pub fn invariant_replay_fail(
        &mut self,
        replayed_entirely: bool,
        invariant_name: &String,
        call_sequence: Vec<BaseCounterExample>,
        stack_trace_result: Option<StackTraceResult<HaltReasonT>>,
        duration: Duration,
    ) {
        self.kind = TestKind::Invariant {
            runs: 1,
            calls: 1,
            reverts: 1,
            metrics: HashMap::default(),
            failed_corpus_replays: 0,
        };
        self.status = TestStatus::Failure;
        self.reason = if replayed_entirely {
            Some(format!("{invariant_name} replay failure"))
        } else {
            Some(format!("{invariant_name} persisted failure revert"))
        };
        self.counterexample = Some(CounterExample::Sequence(call_sequence.len(), call_sequence));
        self.stack_trace_result = stack_trace_result;
        self.duration = duration;
    }

    /// Returns the fail result for invariant test setup.
    pub fn invariant_setup_fail(&mut self, e: InvariantFuzzError, duration: Duration) {
        self.kind = TestKind::Invariant {
            runs: 0,
            calls: 0,
            reverts: 0,
            metrics: HashMap::default(),
            failed_corpus_replays: 0,
        };
        self.status = TestStatus::Failure;
        self.reason = Some(format!(
            "failed to set up invariant testing environment: {e}"
        ));
        self.duration = duration;
    }

    /// Returns the invariant test result.
    #[expect(clippy::too_many_arguments)]
    pub fn invariant_result(
        &mut self,
        gas_report_traces: Vec<Vec<CallTraceArena>>,
        success: bool,
        reason: Option<String>,
        counterexample: Option<CounterExample>,
        cases: Vec<FuzzedCases>,
        reverts: usize,
        metrics: HashMap<String, InvariantMetrics>,
        failed_corpus_replays: usize,
        duration: Duration,
    ) {
        self.kind = TestKind::Invariant {
            runs: cases.len(),
            calls: cases.iter().map(|sequence| sequence.cases().len()).sum(),
            reverts,
            metrics,
            failed_corpus_replays,
        };
        self.status = match success {
            true => TestStatus::Success,
            false => TestStatus::Failure,
        };
        self.reason = reason;
        self.counterexample = counterexample;
        self.gas_report_traces = gas_report_traces;
        self.duration = duration;
    }

    /// Returns `true` if this is the result of a fuzz test
    pub fn is_fuzz(&self) -> bool {
        matches!(self.kind, TestKind::Fuzz { .. })
    }

    /// Formats the test result into a string (for printing).
    pub fn short_result(&self, name: &str) -> String {
        format!("{self} {name} {}", self.kind.report())
    }

    /// Merges the given raw call result into `self`.
    pub fn extend<
        BlockT: BlockEnvTr,
        ChainContextT: 'static + ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        TxT: TransactionEnvTr,
    >(
        &mut self,
        call_result: RawCallResult<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) {
        self.logs.extend(call_result.logs);
        self.decoded_logs = decode_console_logs(&self.logs);
        self.labeled_addresses.extend(call_result.labels);
        self.traces.extend(
            call_result
                .traces
                .map(|traces| (TraceKind::Execution, traces)),
        );
        self.merge_coverages(call_result.line_coverage);
    }

    /// Merges the given coverage result into `self`.
    pub fn merge_coverages(&mut self, other_coverage: Option<HitMaps>) {
        HitMaps::merge_opt(&mut self.line_coverage, other_coverage);
    }
}

/// Data report by a test.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TestKindReport {
    Standard {
        gas: u64,
    },
    Fuzz {
        runs: usize,
        mean_gas: u64,
        median_gas: u64,
    },
    Invariant {
        runs: usize,
        calls: usize,
        reverts: usize,
        metrics: HashMap<String, InvariantMetrics>,
        failed_corpus_replays: usize,
    },
}

impl fmt::Display for TestKindReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TestKindReport::Standard { gas } => {
                write!(f, "(gas: {gas})")
            }
            TestKindReport::Fuzz {
                runs,
                mean_gas,
                median_gas,
            } => {
                write!(f, "(runs: {runs}, μ: {mean_gas}, ~: {median_gas})")
            }
            TestKindReport::Invariant {
                runs,
                calls,
                reverts,
                metrics: _,
                failed_corpus_replays,
            } => {
                if *failed_corpus_replays != 0 {
                    write!(
                        f,
                        "(runs: {runs}, calls: {calls}, reverts: {reverts}, failed corpus replays: {failed_corpus_replays})"
                    )
                } else {
                    write!(f, "(runs: {runs}, calls: {calls}, reverts: {reverts})")
                }
            }
        }
    }
}

impl TestKindReport {
    /// Returns the main gas value to compare against
    pub fn gas(&self) -> u64 {
        match self {
            TestKindReport::Standard { gas } => *gas,
            // We use the median for comparisons
            TestKindReport::Fuzz { median_gas, .. } => *median_gas,
            // We return 0 since it's not applicable
            TestKindReport::Invariant { .. } => 0,
        }
    }
}

/// Various types of tests
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TestKind {
    /// A unit test.
    Unit { gas: u64 },
    /// A solidity fuzz test, that stores all test cases
    Fuzz {
        runs: usize,
        mean_gas: u64,
        median_gas: u64,
    },
    /// A solidity invariant test, that stores all test cases
    Invariant {
        runs: usize,
        calls: usize,
        reverts: usize,
        metrics: HashMap<String, InvariantMetrics>,
        failed_corpus_replays: usize,
    },
}

impl Default for TestKind {
    fn default() -> Self {
        Self::Unit { gas: 0 }
    }
}

impl TestKind {
    /// The gas consumed by this test
    pub fn report(&self) -> TestKindReport {
        match self {
            TestKind::Unit { gas } => TestKindReport::Standard { gas: *gas },
            TestKind::Fuzz {
                runs,
                mean_gas,
                median_gas,
                ..
            } => TestKindReport::Fuzz {
                runs: *runs,
                mean_gas: *mean_gas,
                median_gas: *median_gas,
            },
            TestKind::Invariant {
                runs,
                calls,
                reverts,
                metrics,
                failed_corpus_replays,
            } => TestKindReport::Invariant {
                runs: *runs,
                calls: *calls,
                reverts: *reverts,
                metrics: metrics.clone(),
                failed_corpus_replays: *failed_corpus_replays,
            },
        }
    }
}

#[derive(Clone, Debug)]
#[derive_where(Default)]
pub struct TestSetup<HaltReasonT> {
    /// The address at which the test contract was deployed.
    pub address: Address,
    /// Defined fuzz test fixtures.
    pub fuzz_fixtures: FuzzFixtures,

    /// The logs emitted during setup.
    pub logs: Vec<Log>,
    /// Addresses labeled during setup.
    pub labels: AddressHashMap<String>,
    /// Call traces of the setup.
    pub traces: Traces,
    /// Coverage info during setup.
    pub coverage: Option<HitMaps>,
    /// Addresses of external libraries deployed during setup.
    pub deployed_libs: Vec<Address>,

    /// The reason the setup failed, if it did.
    pub reason: Option<String>,
    /// Whether setup and entire test suite is skipped.
    pub skipped: bool,
    /// Whether the test failed to deploy.
    pub deployment_failure: bool,

    /// The outcome of the stack trace error computation.
    /// None if the test status is succeeded or skipped.
    /// If the heuristic failed the vec is set but emtpy.
    /// Error if there was an error computing the stack trace.
    pub stack_trace_result: Option<StackTraceResult<HaltReasonT>>,
    /// Whether the test had a setup method.
    pub has_setup_method: bool,
    /// Indeterminism from cheatcodes during execution.
    pub indeterminism_reasons: Option<IndeterminismReasons>,
}

impl<HaltReasonT: HaltReasonTrait> TestSetup<HaltReasonT> {
    pub fn failed(reason: String) -> Self {
        Self {
            reason: Some(reason),
            ..Default::default()
        }
    }

    pub fn skipped(reason: String) -> Self {
        Self {
            reason: Some(reason),
            skipped: true,
            ..Default::default()
        }
    }

    pub fn extend<
        BlockT: BlockEnvTr,
        ChainContextT: 'static + ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        TxT: TransactionEnvTr,
    >(
        &mut self,
        raw: RawCallResult<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
        trace_kind: TraceKind,
    ) {
        self.logs.extend(raw.logs);
        self.labels.extend(raw.labels);
        self.traces
            .extend(raw.traces.map(|traces| (trace_kind, traces)));
        if let Some(indeterminism_reasons) = self.indeterminism_reasons.as_mut() {
            indeterminism_reasons.merge(raw.indeterminism_reasons);
        } else {
            self.indeterminism_reasons = raw.indeterminism_reasons;
        }
        HitMaps::merge_opt(&mut self.coverage, raw.line_coverage);
    }
}
