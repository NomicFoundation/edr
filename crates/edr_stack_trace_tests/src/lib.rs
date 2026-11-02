//! Runs the Solidity stack-trace corpus against the provider.
//!
//! Each corpus case is a directory with Solidity sources and a `test.json`
//! listing transactions and the stack trace (and `console.log` output) each
//! one must produce. A [`CompilerConfig`] compiles every case, replays its
//! transactions through [`edr_provider::Provider`], and compares the stack
//! traces EDR infers. The outcome per configuration is checked against a
//! checked-in list of cases known to fail ([`known_failures`]), so a newly
//! failing case and a newly passing one both fail the run.

// The provider's deeply nested async futures exceed the default
// layout-computation recursion limit of 128.
#![recursion_limit = "256"]

mod compare;
mod compiler;
mod console_log;
mod corpus;
mod execution;
pub mod known_failures;

use std::path::{Path, PathBuf};

pub use self::compiler::{CompilerConfig, Optimizer};
use self::corpus::Case;

/// The corpus root, holding the version-grouped case directories.
pub fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../hardhat-tests/test/internal/hardhat-network/stack-traces/test-files")
}

/// The outcome of one configuration run.
#[derive(Debug, Default)]
pub struct RunReport {
    /// Cases that ran and matched their expectations.
    pub passed: Vec<String>,
    /// Cases that ran and failed, with the reason.
    pub failed: Vec<(String, String)>,
    /// Cases the corpus excludes for this configuration.
    pub skipped: Vec<String>,
}

/// Runs every case of `groups` (e.g. `["0_8", "version-independent"]`) under
/// `config`. `EDR_STACK_TRACE_FILTER` restricts the run to case paths
/// containing its value.
pub fn run(config: &CompilerConfig, groups: &[&str]) -> anyhow::Result<RunReport> {
    let compiler = config.install()?;
    let filter = std::env::var("EDR_STACK_TRACE_FILTER").ok();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let _guard = runtime.enter();

    let mut report = RunReport::default();
    for group in groups {
        for case in corpus::discover(&corpus_dir(), group)? {
            if filter
                .as_deref()
                .is_some_and(|filter| !case.name.contains(filter))
            {
                continue;
            }
            run_case(config, &compiler, &case, runtime.handle(), &mut report);
        }
    }
    Ok(report)
}

fn run_case(
    config: &CompilerConfig,
    compiler: &Path,
    case: &Case,
    runtime: &tokio::runtime::Handle,
    report: &mut RunReport,
) {
    if case.is_skipped_for(config) {
        report.skipped.push(case.name.clone());
        return;
    }

    let outcome = config
        .compile(compiler, case)
        .and_then(|compilation| execution::run(case, &compilation, config, runtime));
    match outcome {
        Ok(()) => report.passed.push(case.name.clone()),
        Err(error) => report
            .failed
            .push((case.name.clone(), format!("{error:#}"))),
    }
}
