use std::collections::HashMap;

use alloy_dyn_abi::JsonAbiExt;
use alloy_json_abi::Function;
use alloy_primitives::{Address, Bytes, Log, U256};
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
    FuzzTestResult,
};
use foundry_evm_traces::SparsedTraceArena;
use proptest::{
    strategy::{Strategy, ValueTree},
    test_runner::{TestCaseError, TestRunner},
};
use revm::context::result::{HaltReason, HaltReasonTr};

use crate::executors::{Executor, FuzzTestTimer};

mod types;
pub use types::{CaseOutcome, CounterExampleOutcome, FuzzOutcome};

use crate::executors::fuzz::types::CounterExampleData;

/// Contains data collected during fuzz test runs.
#[derive_where(Default; BlockT, HardforkT, TxT)]
struct FuzzTestData<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
> {
    // Stores the first fuzz case.
    first_case: Option<FuzzCase>,
    // Stored gas usage per fuzz case.
    gas_by_case: Vec<(u64, u64)>,
    // Stores the result and calldata of the last failed call, if any.
    counterexample: CounterExampleData<
        BlockT,
        TxT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
    >,
    // Stores up to `max_traces_to_collect` traces.
    traces: Vec<SparsedTraceArena>,
    // Stores coverage information for all fuzz cases.
    coverage: Option<HitMaps>,
    // Stores logs for all fuzz cases
    logs: Vec<Log>,
    // Deprecated cheatcodes mapped to their replacements.
    deprecated_cheatcodes: HashMap<&'static str, Option<&'static str>>,
    // Runs performed in fuzz test.
    runs: u32,
    // Current assume rejects of the fuzz run.
    rejects: u32,
    // Test failure.
    failure: Option<TestCaseError>,
}

/// Wrapper around an [`Executor`] which provides fuzzing support using
/// [`proptest`].
///
/// After instantiation, calling `fuzz` will proceed to hammer the deployed
/// smart contract with inputs, until it finds a counterexample. The provided
/// [`TestRunner`] contains all the configuration which can be overridden via
/// [environment variables](proptest::test_runner::Config)
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
        Self {
            executor,
            runner,
            sender,
            config,
            persisted_failure,
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
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: 'static
            + EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: 'static + HaltReasonTr + TryInto<HaltReason>,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: 'static + ChainContextTr,
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
    /// Returns a list of all the consumed gas and calldata of every fuzz case.
    pub fn fuzz(
        &mut self,
        func: &Function,
        fuzz_fixtures: &FuzzFixtures,
        deployed_libs: &[Address],
        address: Address,
        rd: &RevertDecoder,
    ) -> FuzzTestResult {
        // Stores the fuzz test execution data.
        let mut test_data = FuzzTestData::default();
        let state = self.build_fuzz_state(deployed_libs);
        let dictionary_weight = self.config.dictionary.dictionary_weight.min(100);
        let strategy = proptest::prop_oneof![
            100 - dictionary_weight => fuzz_calldata(func.clone(), fuzz_fixtures),
            dictionary_weight => fuzz_calldata_from_state(func.clone(), &state),
        ];
        // We want to collect at least one trace which will be displayed to user.
        let max_traces_to_collect = std::cmp::max(1, self.config.gas_report_samples) as usize;

        // Start timer for this fuzz test.
        let timer = FuzzTestTimer::new(self.config.timeout);
        let max_runs = self.config.runs;
        let continue_campaign = |runs: u32| {
            if timer.is_enabled() {
                !timer.is_timed_out()
            } else {
                runs < max_runs
            }
        };

        'stop: while continue_campaign(test_data.runs) {
            // If counterexample recorded, replay it first, without incrementing runs.
            let input = if let Some(failure) = self.persisted_failure.take()
                && failure
                    .calldata
                    .get(..4)
                    .is_some_and(|selector| func.selector() == selector)
            {
                failure.calldata.clone()
            } else {
                test_data.runs += 1;

                match strategy.new_tree(&mut self.runner) {
                    Ok(tree) => tree.current(),
                    Err(err) => {
                        test_data.failure = Some(TestCaseError::fail(format!(
                            "failed to generate fuzzed input: {err}"
                        )));
                        break 'stop;
                    }
                }
            };

            match self.single_fuzz(address, input) {
                Ok(fuzz_outcome) => match fuzz_outcome {
                    FuzzOutcome::Case(case) => {
                        test_data
                            .gas_by_case
                            .push((case.case.gas, case.case.stipend));

                        if test_data.first_case.is_none() {
                            test_data.first_case.replace(case.case);
                        }

                        if let Some(call_traces) = case.call_trace_arena {
                            if test_data.traces.len() == max_traces_to_collect {
                                test_data.traces.pop();
                            }
                            test_data.traces.push(call_traces);
                        }

                        if self.config.show_logs {
                            test_data.logs.extend(case.logs);
                        }

                        HitMaps::merge_opt(&mut test_data.coverage, case.coverage);
                        test_data.deprecated_cheatcodes = case.deprecated_cheatcodes;
                    }
                    FuzzOutcome::CounterExample(CounterExampleOutcome {
                        exit_reason: status,
                        counterexample: outcome,
                    }) => {
                        let reason = rd.maybe_decode(&outcome.call.result, status);
                        test_data.logs.extend(outcome.call.logs.clone());
                        test_data.counterexample = outcome;
                        // HACK: we have to use an empty string here to denote `None`.
                        test_data.failure = Some(TestCaseError::fail(reason.unwrap_or_default()));
                        break 'stop;
                    }
                },
                Err(err) => match err {
                    TestCaseError::Fail(_) => {
                        test_data.failure = Some(err);
                        break 'stop;
                    }
                    TestCaseError::Reject(_) => {
                        // Discard run and apply max rejects if configured. Saturate to handle
                        // the case of replayed failure, which doesn't count as a run.
                        test_data.runs = test_data.runs.saturating_sub(1);
                        if self.config.max_test_rejects > 0 {
                            test_data.rejects += 1;
                            if test_data.rejects >= self.config.max_test_rejects {
                                test_data.failure = Some(TestCaseError::reject(
                                    FuzzError::TooManyRejects(self.config.max_test_rejects),
                                ));
                                break 'stop;
                            }
                        }
                    }
                },
            }
        }

        let CounterExampleData { calldata, call } = test_data.counterexample;

        let mut traces = test_data.traces;
        let last_run_traces = if test_data.failure.is_none() {
            traces.pop()
        } else {
            // Nothing reads `BaseCounterExample::traces`, so the failing
            // arena can move into `FuzzTestResult::call_trace_arena` rather than
            // be cloned for both.
            call.call_trace_arena
        };

        let mut result = FuzzTestResult {
            first_case: test_data.first_case.unwrap_or_default(),
            gas_by_case: test_data.gas_by_case,
            success: test_data.failure.is_none(),
            skipped: false,
            reason: None,
            counterexample: None,
            logs: test_data.logs,
            labeled_addresses: call.labels,
            call_trace_arena: last_run_traces,
            gas_report_traces: traces.into_iter().map(|a| a.arena).collect(),
            line_coverage: test_data.coverage,
            deprecated_cheatcodes: test_data.deprecated_cheatcodes,
        };

        match test_data.failure {
            Some(TestCaseError::Fail(reason)) => {
                let reason = reason.to_string();
                result.reason = (!reason.is_empty()).then_some(reason);
                let args = if let Some(data) = calldata.get(4..) {
                    func.abi_decode_input(data).unwrap_or_default()
                } else {
                    vec![]
                };

                result.counterexample =
                    Some(CounterExample::Single(BaseCounterExample::from_fuzz_call(
                        calldata,
                        &args,
                        // Nothing consumes counterexample arenas; see above.
                        None,
                        call.indeterminism_reasons,
                    )));
            }
            Some(TestCaseError::Reject(reason)) => {
                let reason = reason.to_string();
                result.reason = (!reason.is_empty()).then_some(reason);
            }
            None => {}
        }

        if let Some(reason) = &result.reason
            && let Some(reason) = SkipReason::decode_self(reason)
        {
            result.skipped = true;
            result.reason = reason.0;
        }

        state.log_stats();

        result
    }

    /// Granular and single-step function that runs only one fuzz and returns
    /// either a `CaseOutcome` or a `CounterExampleOutcome`
    #[allow(clippy::type_complexity)]
    fn single_fuzz(
        &self,
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
        let mut call = self
            .executor
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
            self.executor.is_raw_call_mut_success(&mut call, false)
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
