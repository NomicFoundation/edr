//! Configuration types for EDR's Solidity-related functionality.

/// Configuration that controls whether execution traces are decoded and
/// included in results.
///
/// This can either be for Solidity test results or provider transaction
/// execution results.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IncludeTraces {
    /// No traces will be included at all.
    #[default]
    None,
    /// Traces will be included only on the results of failed tests or
    /// execution.
    Failing,
    /// Traces will be included for all test results or executed transactions.
    All,
}

impl IncludeTraces {
    /// Whether traces should be included based on this configuration and the
    /// provided function that indicates whether the execution was a failure.
    pub fn should_include(&self, was_failure_fn: impl FnOnce() -> bool) -> bool {
        match self {
            IncludeTraces::None => false,
            IncludeTraces::Failing => was_failure_fn(),
            IncludeTraces::All => true,
        }
    }
}
