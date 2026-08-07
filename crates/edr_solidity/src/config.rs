//! Configuration types for EDR's Solidity-related functionality.

/// Which results carry call traces — the tree of calls recorded during an
/// execution.
///
/// This can either be for Solidity test results or provider transaction
/// execution results. It says nothing about stack traces: whether a failing
/// Solidity test also gets a source-level stack trace is controlled by
/// `CollectStackTraces`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IncludeCallTraces {
    /// No call traces will be included at all.
    #[default]
    None,
    /// Call traces will be included only on the results of failed tests or
    /// executions.
    Failing,
    /// Call traces will be included for all test results and executed
    /// transactions.
    All,
}

impl IncludeCallTraces {
    /// Whether a result should carry call traces, given a predicate reporting
    /// whether the execution failed. The predicate is only evaluated for
    /// [`Failing`](Self::Failing).
    #[must_use]
    pub fn should_include(self, was_failure_fn: impl FnOnce() -> bool) -> bool {
        match self {
            IncludeCallTraces::None => false,
            IncludeCallTraces::Failing => was_failure_fn(),
            IncludeCallTraces::All => true,
        }
    }
}
