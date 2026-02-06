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
    TimeoutConfig,
};

pub mod result;

pub use foundry_evm::executors::stack_trace::SolidityTestStackTraceError;

mod error;
mod test_filter;

pub use foundry_evm::*;
pub use test_filter::{TestFilter, TestFilterConfig};
