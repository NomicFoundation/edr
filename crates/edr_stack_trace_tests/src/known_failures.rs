//! The checked-in list of cases each configuration is expected to fail.
//!
//! `known-failures/<configuration>.txt` holds one case path per line;
//! anything after `#` is a comment, conventionally the failure's cluster.

use std::{collections::BTreeSet, fmt::Write as _, path::PathBuf};

use crate::{CompilerConfig, RunReport};

fn path(config: &CompilerConfig) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("known-failures")
        .join(format!("{}.txt", config.name()))
}

fn load(config: &CompilerConfig) -> anyhow::Result<BTreeSet<String>> {
    let path = path(config);
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return Ok(BTreeSet::new());
    };
    Ok(contents
        .lines()
        .map(|line| line.split('#').next().unwrap_or_default().trim())
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

/// Fails unless the cases that failed are exactly the listed ones. With
/// `EDR_STACK_TRACE_FILTER` set, listed cases outside the filter are ignored.
pub fn check(config: &CompilerConfig, report: &RunReport) -> anyhow::Result<()> {
    let listed = load(config)?;
    let ran: BTreeSet<&str> = report
        .passed
        .iter()
        .chain(report.failed.iter().map(|(name, _)| name))
        .map(String::as_str)
        .collect();

    let unexpected_failures: Vec<&(String, String)> = report
        .failed
        .iter()
        .filter(|(name, _)| !listed.contains(name))
        .collect();
    let unexpected_passes: Vec<&String> = report
        .passed
        .iter()
        .filter(|name| listed.contains(*name))
        .collect();
    let stale: Vec<&String> = listed
        .iter()
        .filter(|name| !ran.contains(name.as_str()) && !report.skipped.contains(name))
        .filter(|_| std::env::var_os("EDR_STACK_TRACE_FILTER").is_none())
        .collect();

    let mut message = String::new();
    for (name, reason) in &unexpected_failures {
        writeln!(message, "FAILED {name}\n  {}", reason.replace('\n', "\n  "))?;
    }
    for name in &unexpected_passes {
        writeln!(message, "PASSED but listed as a known failure: {name}")?;
    }
    for name in &stale {
        writeln!(
            message,
            "listed as a known failure but not in the corpus: {name}"
        )?;
    }
    anyhow::ensure!(
        message.is_empty(),
        "{}: {} passed, {} failed ({} known), {} skipped; update {} if intended\n{message}",
        config.name(),
        report.passed.len(),
        report.failed.len(),
        report.failed.len() - unexpected_failures.len(),
        report.skipped.len(),
        path(config).display(),
    );
    Ok(())
}
