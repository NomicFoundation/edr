//! Policy deciding which recorded call-trace arenas a finished test still
//! needs.
//!
//! Trace arenas are recorded during execution according to
//! [`TracingMode`](foundry_evm::traces::TracingMode), but which of them are
//! ever *consumed* afterwards depends on `include_traces` (which test results
//! carry call traces to the caller) and `generate_gas_report`. Everything else
//! is dead weight that would otherwise stay resident until the whole test
//! suite completes — with EVM step recording enabled this is the difference
//! between megabytes and gigabytes.

use edr_chain_spec::HaltReasonTrait;
use edr_solidity::config::IncludeTraces;

use crate::result::TestResult;

/// What a finished test's recorded arenas are still needed for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Retain {
    /// Nothing consumes the arenas: free them.
    Nothing,
    /// The call tree is still consumed (trace decoding, gas report, napi
    /// conversion), but recorded EVM steps are not: only the stack-trace
    /// inferrer reads steps, and it has already run by the time a test
    /// finishes.
    CallsOnly,
}

/// Decides which trace arenas to retain once a test has finished and its
/// stack trace (if any) has been computed.
///
/// Derived from the test runner's `include_traces` and `generate_gas_report`
/// settings in `MultiContractRunner::run_test_suite`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TraceRetentionPolicy {
    include_traces: IncludeTraces,
    generate_gas_report: bool,
}

impl TraceRetentionPolicy {
    pub fn new(include_traces: IncludeTraces, generate_gas_report: bool) -> Self {
        Self {
            include_traces,
            generate_gas_report,
        }
    }

    /// What the arenas of a finished test with the given failure status are
    /// still needed for.
    fn retain_after(&self, is_failure: bool) -> Retain {
        if self.include_traces.should_include(|| is_failure) {
            Retain::CallsOnly
        } else {
            Retain::Nothing
        }
    }

    /// Whether the arenas of a test with the given failure status are
    /// consumed at all after it finishes.
    pub fn retains(&self, is_failure: bool) -> bool {
        self.retain_after(is_failure) != Retain::Nothing
    }

    /// Whether a suite's setup arenas are consumed after its tests have
    /// finished. Contract identification and stack-trace code maps are
    /// derived from them *during* the run; afterwards they are only consumed
    /// by trace decoding, the gas report and the napi conversion.
    pub fn retains_setup(&self) -> bool {
        self.include_traces != IncludeTraces::None
    }

    /// Applies the policy to a finished test result, freeing every arena (or
    /// part of one) that no longer has a consumer.
    ///
    /// Must only be called after the test's stack trace has been computed:
    /// stack-trace generation is the one consumer of recorded EVM steps.
    pub fn apply<HaltReasonT: HaltReasonTrait>(&self, result: &mut TestResult<HaltReasonT>) {
        match self.retain_after(result.status.is_failure()) {
            Retain::Nothing => {
                result.execution_traces = Vec::new();
            }
            Retain::CallsOnly => {
                // The call tree is still consumed downstream, but the
                // recorded EVM steps are not; drop them.
                for arena in &mut result.execution_traces {
                    arena.strip_steps();
                }
            }
        }

        // Counterexample arenas have no consumer at all once the test has
        // finished: they are neither decoded nor exposed over napi.
        if let Some(counterexample) = &mut result.counterexample {
            match counterexample {
                crate::fuzz::CounterExample::Single(counterexample) => {
                    counterexample.traces = None;
                }
                crate::fuzz::CounterExample::Sequence(_, counterexamples) => {
                    for counterexample in counterexamples {
                        counterexample.traces = None;
                    }
                }
            }
        }

        // Gas-report samples are only consumed by the gas report.
        if !self.generate_gas_report {
            result.gas_report_traces = Vec::new();
        }
    }
}
