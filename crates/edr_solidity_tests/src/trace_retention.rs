//! Policy for freeing the trace data a finished test — or a finished
//! suite — no longer needs.
//!
//! Arenas are recorded according to
//! [`TracingMode`](foundry_evm::traces::TracingMode) and consumed by
//! stack-trace generation while the test runs. Whether anything consumes them
//! *after* the test has finished depends on `include_traces` — which results
//! carry call traces to the caller — and `generate_gas_report`. Even a
//! retained arena only needs its call tree, never the recorded EVM steps.
//! Without this policy the unconsumed arenas would stay resident until the
//! whole suite completes, which with step recording enabled is the difference
//! between megabytes and gigabytes.

use edr_solidity::config::IncludeTraces;

use crate::result::TestStatus;

/// What a finished test's recorded arenas are still needed for.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Retain {
    /// Nothing consumes the arenas: free them.
    Nothing,
    /// Only the call tree is still consumed — by trace decoding, the gas
    /// report and the napi conversion — so the arenas are kept, but their
    /// recorded EVM steps are dropped.
    CallsOnly,
}

/// Decides which trace arenas to retain once a test has finished and its
/// stack trace (if any) has been computed, and whether a suite's setup
/// arenas outlive its tests.
///
/// Derived from the test runner's `include_traces` and `generate_gas_report`
/// settings in `MultiContractRunner::run_test_suite`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TraceRetentionPolicy {
    /// Which results carry call traces to the caller.
    include_traces: IncludeTraces,
    /// Whether a gas report is being generated, which consumes every test's
    /// call traces.
    generate_gas_report: bool,
}

impl TraceRetentionPolicy {
    /// Creates a policy from the test runner's trace-related settings.
    pub(crate) fn new(include_traces: IncludeTraces, generate_gas_report: bool) -> Self {
        Self {
            include_traces,
            generate_gas_report,
        }
    }

    /// Returns what the arenas of a finished test with the given status are
    /// still needed for.
    pub(crate) fn retain_after(self, status: TestStatus) -> Retain {
        // The gas report consumes every test's call traces.
        // `MultiContractRunner::new` already forces `include_traces` to `All`
        // when one is requested; checking here keeps the policy right on its
        // own.
        if self.generate_gas_report || self.include_traces.should_include(|| status.is_failure()) {
            Retain::CallsOnly
        } else {
            Retain::Nothing
        }
    }

    /// Whether a finished test with the given status still has a consumer for
    /// its arenas — i.e. whether
    /// [`TestResult::free_unconsumed_traces`](crate::result::TestResult::free_unconsumed_traces)
    /// would keep them.
    ///
    /// Lets producers skip accumulating arenas that would only be freed
    /// again. Callers must pass the status the finished result will have, or
    /// a result could lose call traces it should have carried.
    #[must_use]
    pub(crate) fn retains(self, status: TestStatus) -> bool {
        match self.retain_after(status) {
            Retain::Nothing => false,
            Retain::CallsOnly => true,
        }
    }

    /// Whether a suite's setup arenas still have a consumer once its tests
    /// have finished, given whether any of the tests failed. Contract
    /// identification and the deployed-code map used for stack traces are
    /// derived from the arenas before the tests start; afterwards they are
    /// only read by trace decoding and the napi conversion — which surface
    /// them alongside a test result's own traces — and by the gas report,
    /// which analyses them for gas. That is the same set of consumers a
    /// test's own arenas have, so the setup arenas follow the rule for a
    /// failing test if any test failed and for a passing one otherwise.
    #[must_use]
    pub(crate) fn retains_setup(self, any_failed: bool) -> bool {
        self.retains(if any_failed {
            TestStatus::Failure
        } else {
            TestStatus::Success
        })
    }

    /// Returns whether the gas-report samples a test produced still have a
    /// consumer: the gas-report pass in `MultiContractRunner::run_test_suite`.
    pub(crate) fn retains_gas_report_samples(self) -> bool {
        self.generate_gas_report
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::map::HashMap;
    use edr_chain_spec::EvmHaltReason;
    use foundry_evm::traces::{CallTraceArena, SparsedTraceArena};

    use super::*;
    use crate::result::TestResult;

    #[test]
    fn retains_arenas_only_for_results_that_surface_call_traces() {
        let cases = [
            (IncludeTraces::None, TestStatus::Failure, false),
            (IncludeTraces::None, TestStatus::Success, false),
            (IncludeTraces::Failing, TestStatus::Failure, true),
            (IncludeTraces::Failing, TestStatus::Success, false),
            // Skipped tests follow the same rule as passing ones.
            (IncludeTraces::Failing, TestStatus::Skipped, false),
            (IncludeTraces::All, TestStatus::Success, true),
            (IncludeTraces::All, TestStatus::Skipped, true),
        ];
        for (include_traces, status, retained) in cases {
            let policy = TraceRetentionPolicy::new(include_traces, false);
            assert_eq!(
                policy.retains(status),
                retained,
                "{include_traces:?} {status:?}"
            );
            // Setup arenas follow the same rule, keyed on whether any test
            // failed.
            assert_eq!(
                policy.retains_setup(status.is_failure()),
                retained,
                "{include_traces:?} setup arenas, any failed: {}",
                status.is_failure()
            );

            // A gas report consumes every test's call traces.
            let policy = TraceRetentionPolicy::new(include_traces, true);
            assert!(
                policy.retains(status),
                "{include_traces:?} {status:?} with a gas report"
            );
        }
    }

    #[test]
    fn frees_or_strips_execution_traces_per_policy() {
        let arena = || SparsedTraceArena {
            arena: CallTraceArena::default(),
            ignored: HashMap::default(),
        };

        // `All` keeps the call traces.
        let mut kept = TestResult::<EvmHaltReason> {
            execution_traces: [arena()].into_iter().collect(),
            ..TestResult::default()
        };
        kept.free_unconsumed_traces(TraceRetentionPolicy::new(IncludeTraces::All, false));
        assert_eq!(kept.execution_traces.len(), 1);

        // `Failing` frees a passing test's call traces.
        let mut freed = TestResult::<EvmHaltReason> {
            status: TestStatus::Success,
            execution_traces: [arena()].into_iter().collect(),
            ..TestResult::default()
        };
        freed.free_unconsumed_traces(TraceRetentionPolicy::new(IncludeTraces::Failing, false));
        assert!(freed.execution_traces.is_empty());
    }
}
