use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
};

use alloy_dyn_abi::JsonAbiExt;
use alloy_json_abi::Function;
use alloy_primitives::{Address, Log, U256};
use derive_where::derive_where;
use foundry_evm_core::{
    constants::{MAGIC_ASSUME, TEST_TIMEOUT},
    decode::{RevertDecoder, SkipReason},
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
use proptest::test_runner::{TestCaseError, TestError, TestRunner};
use revm::context::result::{HaltReason, HaltReasonTr};

use crate::executors::{Executor, FuzzTestTimer};

mod types;
pub use types::{CaseOutcome, CounterExampleOutcome, FuzzOutcome};

use crate::executors::fuzz::types::CounterExampleData;

/// Contains data collected during fuzz test runs.
#[derive_where(Default; BlockT, HardforkT, TxT)]
pub struct FuzzTestData<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
> {
    // Stores the first fuzz case.
    pub first_case: Option<FuzzCase>,
    // Stored gas usage per fuzz case.
    pub gas_by_case: Vec<(u64, u64)>,
    // Stores the result and calldata of the last failed call, if any.
    pub counterexample: CounterExampleData<
        BlockT,
        TxT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
    >,
    // Stores up to `max_traces_to_collect` traces.
    pub traces: Vec<SparsedTraceArena>,
    // Stores coverage information for all fuzz cases.
    pub coverage: Option<HitMaps>,
    // Stores logs for all fuzz cases
    pub logs: Vec<Log>,
    // Stores gas snapshots for all fuzz cases
    pub gas_snapshots: BTreeMap<String, BTreeMap<String, String>>,
    // Deprecated cheatcodes mapped to their replacements.
    pub deprecated_cheatcodes: HashMap<&'static str, Option<&'static str>>,
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
    /// The EVM executor
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
    /// The account that calls tests
    sender: Address,
    /// The fuzz configuration
    config: FuzzConfig,
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
    ) -> Self {
        Self {
            executor,
            runner,
            sender,
            config,
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
    /// Returns a list of all the consumed gas and calldata of every fuzz case
    pub fn fuzz(
        &self,
        func: &Function,
        fuzz_fixtures: &FuzzFixtures,
        deployed_libs: &[Address],
        address: Address,
        should_fail: bool,
        rd: &RevertDecoder,
    ) -> FuzzTestResult {
        // Stores the fuzz test execution data.
        let execution_data = RefCell::new(FuzzTestData::default());
        let state = self.build_fuzz_state(deployed_libs);
        let dictionary_weight = self.config.dictionary.dictionary_weight.min(100);
        let strategy = proptest::prop_oneof![
            100 - dictionary_weight => fuzz_calldata(func.clone(), fuzz_fixtures),
            dictionary_weight => fuzz_calldata_from_state(func.clone(), &state),
        ];
        // We want to collect at least one trace which will be displayed to user.
        let max_traces_to_collect = std::cmp::max(1, self.config.gas_report_samples) as usize;
        let show_logs = self.config.show_logs;

        // Start timer for this fuzz test.
        let timer = FuzzTestTimer::new(self.config.timeout);

        tracing::debug!(func=?func.name, should_fail, "fuzzing");
        let run_result = self.runner.clone().run(&strategy, |calldata| {
            // Check if the timeout has been reached.
            if timer.is_timed_out() {
                return Err(TestCaseError::fail(TEST_TIMEOUT));
            }

            let fuzz_res = self.single_fuzz(address, should_fail, calldata)?;

            match fuzz_res {
                FuzzOutcome::Case(case) => {
                    let mut data = execution_data.borrow_mut();
                    data.gas_by_case.push((case.case.gas, case.case.stipend));

                    if data.first_case.is_none() {
                        data.first_case.replace(case.case);
                    }

                    if let Some(call_traces) = case.traces {
                        if data.traces.len() == max_traces_to_collect {
                            data.traces.pop();
                        }
                        data.traces.push(call_traces);
                    }

                    if show_logs {
                        data.logs.extend(case.logs);
                    }

                    HitMaps::merge_opt(&mut data.coverage, case.coverage);
                    data.deprecated_cheatcodes = case.deprecated_cheatcodes;

                    Ok(())
                }
                FuzzOutcome::CounterExample(CounterExampleOutcome {
                    exit_reason: status,
                    counterexample: outcome,
                }) => {
                    // We cannot use the calldata returned by the test runner in `TestError::Fail`,
                    // since that input represents the last run case, which may not correspond with
                    // our failure - when a fuzz case fails, proptest will try to run at least one
                    // more case to find a minimal failure case.
                    let reason = rd.maybe_decode(&outcome.call.result, Some(status));
                    execution_data
                        .borrow_mut()
                        .logs
                        .extend(outcome.call.logs.clone());
                    execution_data.borrow_mut().counterexample = outcome;
                    // HACK: we have to use an empty string here to denote `None`.
                    Err(TestCaseError::fail(reason.unwrap_or_default()))
                }
            }
        });

        let fuzz_result = execution_data.into_inner();
        let CounterExampleData {
            calldata,
            call,
            indeterminism_reasons,
        } = fuzz_result.counterexample;

        let mut traces = fuzz_result.traces;
        let last_run_traces = if run_result.is_ok() {
            traces.pop()
        } else {
            call.traces.clone()
        };

        let mut result = FuzzTestResult {
            first_case: fuzz_result.first_case.unwrap_or_default(),
            gas_by_case: fuzz_result.gas_by_case,
            success: run_result.is_ok(),
            skipped: false,
            reason: None,
            counterexample: None,
            logs: fuzz_result.logs,
            deprecated_cheatcodes: fuzz_result.deprecated_cheatcodes,
            labeled_addresses: call.labels,
            traces: last_run_traces,
            gas_report_traces: traces.into_iter().map(|a| a.arena).collect(),
            coverage: fuzz_result.coverage,
        };

        match run_result {
            Ok(()) => {}
            Err(TestError::Abort(reason)) => {
                let msg = reason.message();
                // Currently the only operation that can trigger proptest global rejects is the
                // `vm.assume` cheatcode, thus we surface this info to the user when the fuzz
                // test aborts due to too many global rejects, making the error
                // message more actionable.
                result.reason = if msg == "Too many global rejects" {
                    let error = FuzzError::TooManyRejects(self.runner.config().max_global_rejects);
                    Some(error.to_string())
                } else {
                    Some(msg.to_string())
                };
            }
            Err(TestError::Fail(reason, _)) => {
                let reason = reason.to_string();
                if reason == TEST_TIMEOUT {
                    // If the reason is a timeout, we consider the fuzz test successful.
                    result.success = true;
                } else {
                    result.reason = (!reason.is_empty()).then_some(reason);
                    let args = if let Some(data) = calldata.get(4..) {
                        func.abi_decode_input(data).unwrap_or_default()
                    } else {
                        vec![]
                    };

                    result.counterexample =
                        Some(CounterExample::Single(BaseCounterExample::from_fuzz_call(
                            calldata,
                            args,
                            call.traces,
                            indeterminism_reasons,
                        )));
                }
            }
        }

        if let Some(reason) = &result.reason {
            if let Some(reason) = SkipReason::decode_self(reason) {
                result.skipped = true;
                result.reason = reason.0;
            }
        }

        state.log_stats();

        result
    }

    /// Granular and single-step function that runs only one fuzz and returns
    /// either a `CaseOutcome` or a `CounterExampleOutcome`
    #[allow(clippy::type_complexity)]
    pub fn single_fuzz(
        &self,
        address: Address,
        should_fail: bool,
        calldata: alloy_primitives::Bytes,
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
        let (mut call, cow_backend) = self
            .executor
            .call_raw(self.sender, address, calldata.clone(), U256::ZERO)
            .map_err(|e| TestCaseError::fail(e.to_string()))?;

        // Handle `vm.assume`.
        if call.result.as_ref() == MAGIC_ASSUME {
            return Err(TestCaseError::reject(FuzzError::AssumeReject));
        }

        let deprecated_cheatcodes = call
            .cheatcodes
            .as_ref()
            .map_or_else(Default::default, |cheatcodes| cheatcodes.deprecated.clone());

        let success = self
            .executor
            .is_raw_call_mut_success(address, &mut call, should_fail);
        if success {
            Ok(FuzzOutcome::Case(CaseOutcome {
                case: FuzzCase {
                    calldata,
                    gas: call.gas_used,
                    stipend: call.stipend,
                },
                traces: call.traces,
                coverage: call.coverage,
                logs: call.logs,
                deprecated_cheatcodes,
            }))
        } else {
            Ok(FuzzOutcome::CounterExample(CounterExampleOutcome {
                exit_reason: call.exit_reason,
                counterexample: CounterExampleData {
                    calldata,
                    call,
                    indeterminism_reasons: cow_backend.backend.indeterminism_reasons(),
                },
            }))
        }
    }
}
