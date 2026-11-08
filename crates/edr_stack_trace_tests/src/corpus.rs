//! Corpus discovery and the `test.json` schema.

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context as _;
use serde::Deserialize;
use serde_json::Value;

use crate::CompilerConfig;

/// One corpus case: a directory with a `test.json`.
#[derive(Debug)]
pub(crate) struct Case {
    /// Path relative to the corpus root, e.g. `0_8/call-message/...`.
    pub name: String,
    pub definition: TestDefinition,
    /// The case's own `.sol` files plus the first transaction's `imports`.
    pub sources: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestDefinition {
    pub transactions: Vec<Transaction>,
    /// A semver range (node-semver syntax) of compiler versions to run under.
    pub solc: Option<String>,
    #[serde(default)]
    pub skip: bool,
    #[serde(default, rename = "skipViaIR")]
    pub skip_via_ir: bool,
}

/// A deployment when `file` is set, a call to the contract deployed by
/// transaction `to` otherwise.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Transaction {
    pub file: Option<String>,
    pub contract: Option<String>,
    #[serde(default)]
    pub imports: Vec<String>,
    /// `{file: {library: index of the deploying transaction}}`
    #[serde(default)]
    pub libraries: std::collections::BTreeMap<String, std::collections::BTreeMap<String, usize>>,
    pub to: Option<usize>,
    pub function: Option<String>,
    #[serde(default)]
    pub params: Vec<Value>,
    /// Raw calldata; takes precedence over `function`.
    pub data: Option<String>,
    pub value: Option<Value>,
    pub gas: Option<Value>,
    pub stack_trace: Option<Vec<ExpectedEntry>>,
    pub console_logs: Option<Vec<Vec<Value>>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExpectedEntry {
    pub r#type: String,
    pub source_reference: Option<ExpectedSourceReference>,
    pub message: Option<String>,
    pub value: Option<Value>,
    /// Lowercase hex without `0x`.
    pub error_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ExpectedSourceReference {
    pub contract: Option<String>,
    pub file: String,
    pub function: Option<String>,
    pub line: u32,
}

impl Case {
    pub fn is_skipped_for(&self, config: &CompilerConfig) -> bool {
        let definition = &self.definition;
        definition.skip
            || (definition.skip_via_ir && config.is_via_ir())
            || definition
                .solc
                .as_deref()
                .is_some_and(|range| !node_semver_satisfies(range, config.solidity_version()))
    }
}

/// Finds every case under `root/group`, sorted by name.
pub(crate) fn discover(root: &Path, group: &str) -> anyhow::Result<Vec<Case>> {
    let mut cases = Vec::new();
    collect(root, &root.join(group), &mut cases)?;
    cases.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(cases)
}

fn collect(root: &Path, dir: &Path, cases: &mut Vec<Case>) -> anyhow::Result<()> {
    let mut sources = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            collect(root, &path, cases)?;
        } else if path.extension().is_some_and(|extension| extension == "sol") {
            sources.push(path);
        }
    }

    let test_json = dir.join("test.json");
    if !test_json.exists() {
        return Ok(());
    }
    let definition: TestDefinition = serde_json::from_str(&fs::read_to_string(&test_json)?)
        .with_context(|| format!("parsing {}", test_json.display()))?;
    if let Some(imports) = definition
        .transactions
        .iter()
        .map(|transaction| &transaction.imports)
        .find(|imports| !imports.is_empty())
    {
        // Imports are written as `/../../console.sol`, relative to the case directory.
        sources.extend(
            imports
                .iter()
                .map(|import| dir.join(import.trim_start_matches('/'))),
        );
    }
    sources.sort();

    cases.push(Case {
        name: dir.strip_prefix(root)?.to_string_lossy().replace('\\', "/"),
        definition,
        sources,
    });
    Ok(())
}

/// Evaluates a node-semver range such as `>=0.6.3 <0.6.9` or
/// `<0.6.3 || >= 0.6.9`, which `semver::VersionReq` doesn't parse as is.
fn node_semver_satisfies(range: &str, version: &semver::Version) -> bool {
    range.split("||").any(|alternative| {
        let mut comparators: Vec<String> = Vec::new();
        let mut pending_operator = String::new();
        for token in alternative.split_whitespace() {
            if token.chars().all(|c| "<>=~^".contains(c)) {
                pending_operator.push_str(token);
            } else {
                comparators.push(format!("{}{token}", std::mem::take(&mut pending_operator)));
            }
        }
        semver::VersionReq::parse(&comparators.join(","))
            .unwrap_or_else(|error| panic!("invalid solc range {range:?}: {error}"))
            .matches(version)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_semver_ranges() {
        let version = |v: &str| semver::Version::parse(v).unwrap();
        assert!(node_semver_satisfies(">=0.6.3 <0.6.9", &version("0.6.3")));
        assert!(!node_semver_satisfies(">=0.6.3 <0.6.9", &version("0.6.9")));
        assert!(node_semver_satisfies(
            "<0.6.3 || >= 0.6.9",
            &version("0.6.12")
        ));
        assert!(!node_semver_satisfies(
            "<0.6.3 || >= 0.6.9",
            &version("0.6.5")
        ));
        assert!(node_semver_satisfies("^0.8.4", &version("0.8.35")));
    }
}
