#[macro_use]
extern crate tracing;

pub mod gas_report;

pub mod multi_runner;
pub use multi_runner::MultiContractRunner;

mod runner;
pub use runner::ContractRunner;

mod config;
pub use config::{
    CollectStackTraces, FuzzConfigOverride, InvariantConfigOverride, SolidityTestRunnerConfig,
    SolidityTestRunnerConfigError, SyncOnCollectedCoverageCallback, TestFunctionConfigOverride,
    TimeoutConfig, MAX_TEST_TRANSACTION_GAS_LIMIT,
};

pub mod inline_config;
pub mod test_source_error;

mod test_sources;

pub mod result;

pub use foundry_evm::executors::stack_trace::SolidityTestStackTraceError;

pub mod error;
mod test_filter;
mod trace_retention;

pub use foundry_evm::*;
pub use test_filter::{TestFilter, TestFilterConfig};
