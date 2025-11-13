#[macro_use]
extern crate tracing;

pub mod gas_report;

pub mod multi_runner;
pub use multi_runner::MultiContractRunner;

mod runner;
pub use runner::ContractRunner;

mod config;
pub use config::{
    CollectStackTraces, IncludeTraces, SolidityTestRunnerConfig, SolidityTestRunnerConfigError,
    SyncOnCollectedCoverageCallback,
};

pub mod result;

pub use foundry_evm::executors::stack_trace::StackTraceError;

mod error;
mod test_filter;

pub use foundry_evm::*;
pub use test_filter::{TestFilter, TestFilterConfig};
