//! Construction of the inline-config provider from the Hardhat-provided source
//! paths and import mappings.

use std::collections::{BTreeMap, HashMap};

use edr_artifact::ArtifactId;
use edr_napi_core::solidity::config::TestRunnerConfig;
use edr_solidity_tests::{
    inline_config::{ImportResolver, InlineConfigRoot, SharedInlineConfigProvider},
    multi_runner::TestContract,
};

/// Collects the test contracts' inline configuration, reading each test source
/// — and its imports — from disk.
///
/// The absolute path of each test source, and of every non-relative import, is
/// provided by the caller through
/// [`test_source_paths`](TestRunnerConfig::test_source_paths) and
/// [`import_mappings`](TestRunnerConfig::import_mappings): solc source names
/// are logical identifiers (e.g. `project/test/Foo.t.sol`,
/// `npm/forge-std@1.14.0/src/Test.sol`), so they cannot be reliably mapped to
/// disk locations here. A test source without a provided path has no inline
/// configuration collected.
///
/// This uses the synchronous [`SharedInlineConfigProvider::collect`], so
/// collection happens here (blocking) and a failure surfaces immediately.
pub(crate) fn collect_inline_configs(
    config: &TestRunnerConfig,
    test_contracts: &BTreeMap<ArtifactId, TestContract>,
) -> napi::Result<SharedInlineConfigProvider> {
    // Deduplicate by source: a source declaring multiple test contracts is
    // parsed once.
    let mut roots_by_source = HashMap::new();
    for artifact_id in test_contracts.keys() {
        if let Some(path) = config.test_source_paths.get(&artifact_id.source) {
            roots_by_source
                .entry(artifact_id.source.clone())
                .or_insert_with(|| InlineConfigRoot {
                    source: artifact_id.source.clone(),
                    path: path.clone(),
                    version: artifact_id.version.clone(),
                });
        }
    }

    let import_resolver = ImportResolver::new(config.import_mappings.clone());
    SharedInlineConfigProvider::collect(roots_by_source.into_values().collect(), import_resolver)
        .map_err(|error| {
            napi::Error::new(
                napi::Status::GenericFailure,
                format!("Failed to collect inline configuration: {error}"),
            )
        })
}
