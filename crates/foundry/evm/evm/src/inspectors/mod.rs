//! EVM inspectors.

pub use foundry_cheatcodes::{self as cheatcodes, Cheatcodes, CheatsConfig};
pub use foundry_evm_coverage::LineCoverageCollector;
pub use foundry_evm_fuzz::Fuzzer;
pub use foundry_evm_traces::{
    StackSnapshotType, TracingInspector, TracingInspectorConfig, TracingMode,
};
pub use revm_inspectors::access_list::AccessListInspector;

mod error_ext;

mod logs;
pub use logs::LogCollector;

mod stack;

pub use stack::{InspectorData, InspectorStack, InspectorStackBuilder, InspectorStackRefMut};

mod revert_diagnostic;
pub use revert_diagnostic::RevertDiagnostic;
