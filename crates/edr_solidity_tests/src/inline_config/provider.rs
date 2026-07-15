//! Inline-config resolution.
//!
//! Collection: given the set of test sources to cover — each with the absolute
//! path of its file on disk — parse each with Slang and extract every
//! contract's inline configuration. A malformed directive (or an unreadable
//! root file, or an unsupported solc version) fails collection outright, which
//! aborts the whole run before any test executes.
//!
//! Query: look up the precomputed inline configuration for one test contract —
//! a plain map lookup, since all parsing happened during collection.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use rayon::prelude::*;
use semver::Version;

use super::{
    directives,
    error::{InlineConfigCollectError, InlineConfigError},
    overrides::{collect_source, FunctionOverride, SourceOverrides},
    resolver::ImportResolver,
};

/// A Solidity test source to collect inline configuration from.
#[derive(Clone, Debug)]
pub struct InlineConfigRoot {
    /// The identity this root is queried by — the compiled artifact's solc
    /// source name (e.g. the running test contract's `source`). Collections are
    /// keyed by this, because that is what a query has in hand.
    pub source: PathBuf,
    /// Absolute path to the file on disk, used to read and parse it.
    pub path: PathBuf,
    /// The solc version the file was compiled with.
    pub version: Version,
}

/// Caches the fully-parsed inline configuration of every test contract, keyed
/// by source name, so a query is a plain lookup.
///
/// [`collect`](Self::collect) does all the work — read each source and its
/// imports from disk, parse them with Slang, and extract every contract's
/// per-function overrides — once. Only sources that carry a directive are
/// parsed. Any problem (a malformed directive, an unreadable root file, or an
/// unsupported solc version) fails collection.
#[derive(Debug)]
pub struct CachedInlineConfigProvider {
    by_source: HashMap<PathBuf, SourceOverrides>,
}

impl CachedInlineConfigProvider {
    /// Parses every root's inline configuration in parallel, reading each
    /// root's file — and its imports, resolved by `import_resolver` — from
    /// disk. Sources that carry no inline-config directive are skipped. Any
    /// malformed directive, unreadable root file, or unsupported solc
    /// version fails the whole collection.
    pub fn collect(
        roots: &[InlineConfigRoot],
        import_resolver: &ImportResolver,
    ) -> Result<Self, InlineConfigError> {
        let parse =
            |root: &InlineConfigRoot| -> Result<Option<(PathBuf, SourceOverrides)>, InlineConfigError> {
                let content = std::fs::read_to_string(&root.path).map_err(|error| {
                    InlineConfigError::Collect(InlineConfigCollectError::RootFileNotFound {
                        path: root.path.display().to_string(),
                        reason: error.to_string(),
                    })
                })?;
                // Fast path: only parse sources that carry a directive.
                if !directives::contains_inline_config_directive(&content) {
                    return Ok(None);
                }
                let overrides = collect_source(
                    &root.path,
                    Arc::from(content),
                    root.version.clone(),
                    import_resolver,
                )?;
                Ok(Some((root.source.clone(), overrides)))
            };

        // Parse the roots in parallel on rayon's global pool. Collection runs
        // synchronously and completes before any test suite is dispatched, so it
        // never contends with suite execution.
        let collected: Vec<Option<(PathBuf, SourceOverrides)>> =
            roots.par_iter().map(parse).collect::<Result<_, _>>()?;

        Ok(Self {
            by_source: collected.into_iter().flatten().collect(),
        })
    }

    /// Returns the inline configuration of every test function declared
    /// directly in `contract_name` within `source`, as computed during
    /// collection.
    ///
    /// Returns an empty vector if the contract carries no inline configuration.
    /// This is infallible: any malformed directive already failed collection.
    pub fn get(&self, source: &Path, contract_name: &str) -> Vec<FunctionOverride> {
        self.by_source
            .get(source)
            .and_then(|configs| configs.get(contract_name))
            .cloned()
            .unwrap_or_default()
    }
}

/// A cloneable, `Send + Sync` handle to the collected inline configuration,
/// shared across the test runner's parallel suite dispatch.
#[derive(Clone, Debug)]
pub struct SharedInlineConfigProvider(Arc<CachedInlineConfigProvider>);

impl SharedInlineConfigProvider {
    /// Collects every root's inline configuration, returning a shared handle to
    /// the result. A collection failure (a malformed directive, an unreadable
    /// root file, or an unsupported solc version) is returned to the caller,
    /// which aborts the whole run.
    pub fn collect(
        roots: Vec<InlineConfigRoot>,
        import_resolver: ImportResolver,
    ) -> Result<Self, InlineConfigError> {
        let provider = CachedInlineConfigProvider::collect(&roots, &import_resolver)?;
        Ok(Self(Arc::new(provider)))
    }

    /// Returns the inline configuration of `contract_name` within `source` — a
    /// plain lookup against the already-collected configuration.
    pub fn get(&self, source: &Path, contract_name: &str) -> Vec<FunctionOverride> {
        self.0.get(source, contract_name)
    }
}

#[cfg(test)]
mod tests {
    use std::{io::Write as _, thread};

    use super::*;

    const SOURCE_NAME: &str = "project/test.sol";
    const SOURCE: &str = r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract MyTest {
    uint256 internal value;

    /// forge-config: default.fuzz.runs = 42
    function testFuzz(uint256 x) public { value = x; }

    /// hardhat-config: isolate = true
    function testUnit() public {}

    // not natspec: forge-config: default.fuzz.runs = 999
    function testNoConfig() public {}
}
"#;

    /// Writes `source` to a temporary `.sol` file and returns a root under the
    /// query identity `project/test.sol`, together with the handle keeping the
    /// file alive.
    fn root_for(source: &str) -> (InlineConfigRoot, tempfile::NamedTempFile) {
        let mut file = tempfile::Builder::new()
            .suffix(".sol")
            .tempfile()
            .expect("temp file");
        file.write_all(source.as_bytes()).expect("write source");

        let root = InlineConfigRoot {
            source: PathBuf::from(SOURCE_NAME),
            path: file.path().to_path_buf(),
            version: Version::new(0, 8, 0),
        };
        (root, file)
    }

    fn root_with_source() -> (InlineConfigRoot, tempfile::NamedTempFile) {
        root_for(SOURCE)
    }

    fn assert_overrides(overrides: &[FunctionOverride]) {
        assert_eq!(overrides.len(), 2, "{overrides:#?}");
        let fuzz = overrides
            .iter()
            .find(|o| o.function_name == "testFuzz")
            .expect("testFuzz");
        assert_eq!(fuzz.config.fuzz.as_ref().unwrap().runs, Some(42));
        let unit = overrides
            .iter()
            .find(|o| o.function_name == "testUnit")
            .expect("testUnit");
        assert_eq!(unit.config.isolate, Some(true));
        assert!(overrides.iter().all(|o| o.function_name != "testNoConfig"));
    }

    #[test]
    fn cached_collects_and_queries() {
        let (root, _file) = root_with_source();
        let provider = CachedInlineConfigProvider::collect(&[root], &ImportResolver::default())
            .expect("collect succeeds");

        assert_overrides(&provider.get(Path::new(SOURCE_NAME), "MyTest"));
        // A source that was never collected reports no overrides.
        assert!(provider.get(Path::new("never.sol"), "MyTest").is_empty());
    }

    #[test]
    fn collect_fails_on_missing_root_file() {
        let root = InlineConfigRoot {
            source: PathBuf::from(SOURCE_NAME),
            path: PathBuf::from("/nonexistent/test.sol"),
            version: Version::new(0, 8, 0),
        };

        let error = CachedInlineConfigProvider::collect(&[root], &ImportResolver::default())
            .expect_err("missing root file fails collection");
        assert!(matches!(
            error,
            InlineConfigError::Collect(InlineConfigCollectError::RootFileNotFound { .. })
        ));
    }

    #[test]
    fn collect_fails_on_malformed_directive() {
        let source = r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract BadTest {
    /// forge-config: default.fuzz.bogus = 1
    function testFoo(uint256 x) public {}
}
"#;
        let (root, _file) = root_for(source);

        // A malformed directive fails the whole collection rather than being
        // isolated to the offending contract.
        let error = CachedInlineConfigProvider::collect(&[root], &ImportResolver::default())
            .expect_err("malformed directive fails collection");
        assert!(
            matches!(error, InlineConfigError::InvalidKey { .. }),
            "{error:?}"
        );
    }

    #[test]
    fn shared_collect_then_serves() {
        let (root, _file) = root_with_source();
        let provider = SharedInlineConfigProvider::collect(vec![root], ImportResolver::default())
            .expect("collect succeeds");

        assert_overrides(&provider.get(Path::new(SOURCE_NAME), "MyTest"));
    }

    #[test]
    fn shared_serves_concurrent_queries() {
        let (root, _file) = root_with_source();
        let provider = SharedInlineConfigProvider::collect(vec![root], ImportResolver::default())
            .expect("collect succeeds");

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let provider = provider.clone();
                thread::spawn(move || provider.get(Path::new(SOURCE_NAME), "MyTest"))
            })
            .collect();

        for handle in handles {
            assert_overrides(&handle.join().unwrap());
        }
    }

    #[test]
    fn shared_collect_fails_on_malformed_directive() {
        let source = r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract BadTest {
    /// forge-config: default.fuzz.runs = -1
    function testFoo(uint256 x) public {}
}
"#;
        let (root, _file) = root_for(source);

        let error = SharedInlineConfigProvider::collect(vec![root], ImportResolver::default())
            .expect_err("malformed directive fails collection");
        assert!(
            matches!(error, InlineConfigError::InvalidValue { .. }),
            "{error:?}"
        );
    }
}
