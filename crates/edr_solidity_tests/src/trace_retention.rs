//! Policy for freeing the trace data a finished test no longer needs.
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
    /// Only the call tree is still consumed, so the arenas are kept, but
    /// their recorded EVM steps are dropped.
    CallsOnly,
}

/// Decides which trace arenas to retain once a test has finished and its
/// stack trace (if any) has been computed.
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

    /// Returns whether the gas-report samples a test produced still have a
    /// consumer: the gas-report pass in `MultiContractRunner::run_test_suite`.
    pub(crate) fn retains_gas_report_samples(self) -> bool {
        self.generate_gas_report
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{map::HashMap, Bytes};
    use edr_chain_spec::EvmHaltReason;
    use foundry_evm::traces::{CallTraceArena, SparsedTraceArena};

    use super::*;
    use crate::{
        fuzz::{BaseCounterExample, CounterExample},
        result::TestResult,
    };

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
                matches!(policy.retain_after(status), Retain::CallsOnly),
                retained,
                "{include_traces:?} {status:?}"
            );

            // A gas report consumes every test's call traces.
            let policy = TraceRetentionPolicy::new(include_traces, true);
            assert!(
                matches!(policy.retain_after(status), Retain::CallsOnly),
                "{include_traces:?} {status:?} with a gas report"
            );
        }
    }

    #[test]
    fn frees_counterexample_arenas_without_consumers() {
        let arena = || SparsedTraceArena {
            arena: CallTraceArena::default(),
            ignored: HashMap::default(),
        };
        let counterexample =
            || BaseCounterExample::from_fuzz_call(Bytes::new(), &[], Some(arena()), None);

        let mut result = TestResult::<EvmHaltReason> {
            execution_traces: [arena()].into_iter().collect(),
            counterexample: Some(CounterExample::Sequence(
                2,
                vec![counterexample(), counterexample()],
            )),
            ..TestResult::default()
        };

        // `All` keeps the call traces, but the counterexample arenas have no
        // consumer.
        result.free_unconsumed_traces(TraceRetentionPolicy::new(IncludeTraces::All, false));

        assert_eq!(result.execution_traces.len(), 1);
        let Some(CounterExample::Sequence(_, counterexamples)) = &result.counterexample else {
            panic!("counterexample kind changed");
        };
        assert!(counterexamples
            .iter()
            .all(|counterexample| counterexample.traces.is_none()));
    }
}
