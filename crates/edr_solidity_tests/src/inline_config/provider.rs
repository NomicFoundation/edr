//! Two-phase inline-config resolution.
//!
//! Phase 1 (collection): given the set of test sources to cover — each with the
//! absolute path of its file on disk — parse each with Slang and extract every
//! contract's inline configuration. [`CachedInlineConfigProvider`] does this
//! eagerly and synchronously; [`SharedInlineConfigProvider`] wraps it, running
//! the per-root work in parallel on a background thread.
//!
//! Phase 2 (query): look up the precomputed inline configuration for one test
//! contract. For [`SharedInlineConfigProvider`], a query blocks until
//! background collection has finished.

use std::{
    collections::HashMap,
    convert::Infallible,
    path::{Path, PathBuf},
    sync::Arc,
};

use crossbeam_channel::select_biased;
use edr_utils_sync::CancellableThread;
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
/// [`collect`](Self::collect) does all the work — parse each source's text
/// with Slang and extract every contract's per-function overrides — once
/// (eager and synchronous here; [`SharedInlineConfigProvider`] runs it in the
/// background). Only sources that carry a directive are parsed. A malformed
/// directive is stored as the offending contract's error and surfaces only when
/// that contract is queried, rather than failing collection for the whole run.
#[derive(Debug, Default)]
pub struct CachedInlineConfigProvider {
    by_source: HashMap<PathBuf, SourceOverrides>,
}

impl CachedInlineConfigProvider {
    /// Parses every root's inline configuration in parallel, reading each
    /// root's file — and its imports, resolved by `import_resolver` — from
    /// disk. Sources that carry no inline-config directive are skipped; a root
    /// file that can't be read, or whose solc version is unsupported, fails the
    /// whole collection. All directive parsing happens here, so queries are a
    /// plain lookup.
    pub fn collect(
        roots: &[InlineConfigRoot],
        import_resolver: &ImportResolver,
    ) -> Result<Self, InlineConfigCollectError> {
        let parse =
            |root: &InlineConfigRoot| -> Result<Option<(PathBuf, SourceOverrides)>, InlineConfigCollectError> {
                let content = std::fs::read_to_string(&root.path).map_err(|error| {
                    InlineConfigCollectError::RootFileNotFound {
                        path: root.path.display().to_string(),
                        reason: error.to_string(),
                    }
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

        // Parse in parallel, but on a *dedicated* thread pool rather than
        // rayon's global pool.
        //
        // The test runner dispatches its suites on the global rayon pool, and
        // each suite blocks on `SharedInlineConfigProvider::get` until this
        // collection has finished. If the collection also ran on the global
        // pool, those blocked suite workers and this parsing work would compete
        // for the very same threads; with enough suites in flight the pool
        // deadlocks — every worker parked waiting for a collection that has no
        // worker left to run on. A dedicated pool keeps collection independent,
        // so it always makes progress and the blocked queries are released
        // promptly. If the pool can't be built (e.g. thread exhaustion under
        // heavy load), fall back to a sequential parse, which is correct and
        // likewise free of the global-pool dependency.
        let collected: Vec<Option<(PathBuf, SourceOverrides)>> =
            match build_collection_pool(roots.len()) {
                Some(pool) => {
                    pool.install(|| roots.par_iter().map(parse).collect::<Result<_, _>>())?
                }
                None => roots.iter().map(parse).collect::<Result<_, _>>()?,
            };

        Ok(Self {
            by_source: collected.into_iter().flatten().collect(),
        })
    }

    /// Returns the inline configuration of every test function declared
    /// directly in `contract_name` within `source`, as computed during
    /// collection.
    ///
    /// Returns an empty vector if the contract carries no inline configuration,
    /// or an error if one of its directives is malformed.
    pub fn get(
        &self,
        source: &Path,
        contract_name: &str,
    ) -> Result<Vec<FunctionOverride>, InlineConfigError> {
        match self
            .by_source
            .get(source)
            .and_then(|configs| configs.get(contract_name))
        {
            Some(result) => result.clone(),
            None => Ok(Vec::new()),
        }
    }
}

/// Builds a dedicated thread pool for parsing the collection's roots, isolated
/// from rayon's global pool (see [`CachedInlineConfigProvider::collect`]).
///
/// Sized to the available parallelism, but never to more threads than there are
/// roots to parse. Returns `None` if the pool can't be built, in which case the
/// caller parses sequentially.
fn build_collection_pool(root_count: usize) -> Option<rayon::ThreadPool> {
    let threads = std::thread::available_parallelism()
        .map_or(1, std::num::NonZero::get)
        .min(root_count.max(1));

    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|index| format!("inline-config-collect-{index}"))
        .build()
        .ok()
}

/// A single query sent to the background collection thread.
struct InlineConfigRequest {
    source: PathBuf,
    contract_name: String,
    response_sender: crossbeam_channel::Sender<Result<Vec<FunctionOverride>, InlineConfigError>>,
}

/// A cloneable, `Send + Sync` handle that collects inline configuration in the
/// background, parallelizing the per-root parses (via [`rayon`]). Queries block
/// until collection has finished.
#[derive(Clone, Debug)]
pub struct SharedInlineConfigProvider {
    request_sender: crossbeam_channel::Sender<InlineConfigRequest>,
    // Dropping this disconnects the cancellation channel and joins the
    // dedicated thread, so no explicit shutdown is needed.
    _thread: Arc<CancellableThread>,
}

impl SharedInlineConfigProvider {
    const THREAD_NAME: &'static str = "inline-config-provider";

    /// Collects every root's inline configuration synchronously on the calling
    /// thread, then spawns a background thread that only *serves* queries from
    /// the finished [`CachedInlineConfigProvider`].
    ///
    /// A collection failure (an unreadable root file, or a source whose solc
    /// version is unsupported) is returned immediately, before the handle is
    /// created. Contrast
    /// [`collect_in_background`](Self::collect_in_background),
    /// which returns instantly and does the collection off-thread.
    pub fn collect(
        roots: Vec<InlineConfigRoot>,
        import_resolver: ImportResolver,
    ) -> Result<Self, InlineConfigCollectError> {
        let collected = CachedInlineConfigProvider::collect(&roots, &import_resolver)?;

        let (request_sender, request_receiver) = crossbeam_channel::unbounded();
        let thread = CancellableThread::spawn(Self::THREAD_NAME.to_owned(), move |cancellation| {
            Self::serve(Ok(collected), &request_receiver, &cancellation);
        })
        .expect("inline-config provider thread should spawn");

        Ok(Self {
            request_sender,
            _thread: Arc::new(thread),
        })
    }

    /// Spawns a background thread that collects every root in parallel and then
    /// makes the resulting [`CachedInlineConfigProvider`] available to queries.
    ///
    /// If collection itself fails (an unreadable root file, or a source whose
    /// solc version is unsupported), every query returns that failure as an
    /// [`InlineConfigError::Collect`], so the offending run surfaces the error
    /// rather than silently dropping inline configuration.
    pub fn collect_in_background(
        roots: Vec<InlineConfigRoot>,
        import_resolver: ImportResolver,
    ) -> Self {
        let (request_sender, request_receiver) = crossbeam_channel::unbounded();

        let thread = CancellableThread::spawn(Self::THREAD_NAME.to_owned(), move |cancellation| {
            let collected = CachedInlineConfigProvider::collect(&roots, &import_resolver);
            Self::serve(collected, &request_receiver, &cancellation);
        })
        .expect("inline-config provider thread should spawn");

        Self {
            request_sender,
            _thread: Arc::new(thread),
        }
    }

    /// Serves queries against `collected` until the handle is dropped
    /// (cancellation) or every request sender is dropped. A collection failure
    /// is served to every query as an [`InlineConfigError::Collect`].
    fn serve(
        collected: Result<CachedInlineConfigProvider, InlineConfigCollectError>,
        request_receiver: &crossbeam_channel::Receiver<InlineConfigRequest>,
        cancellation: &crossbeam_channel::Receiver<Infallible>,
    ) {
        loop {
            // `select_biased!` picks the first listed branch when multiple arms
            // are ready, so cancellation always wins over pending work.
            select_biased! {
                // The cancellation channel was disconnected by dropping the
                // `CancellableThread`.
                recv(cancellation) -> _ => break,
                recv(request_receiver) -> message => match message {
                    Ok(InlineConfigRequest { source, contract_name, response_sender }) => {
                        let response = match &collected {
                            Ok(provider) => provider.get(&source, &contract_name),
                            // Collection failed; serve the error to every query.
                            Err(error) => Err(InlineConfigError::Collect(error.clone())),
                        };
                        // The caller may have stopped waiting; ignore a
                        // disconnected response channel.
                        let _ = response_sender.send(response);
                    }
                    // All request senders were dropped.
                    Err(_) => break,
                },
            }
        }
    }

    /// Extracts the inline configuration of `contract_name` within `source`,
    /// blocking until background collection has finished.
    ///
    /// This is called from the test runner's suite dispatch, which runs on
    /// rayon's global pool — so this blocks a global-pool worker until the
    /// background collection completes. That is only deadlock-free because the
    /// collection runs on its own dedicated pool and therefore never needs the
    /// workers parked here; see [`CachedInlineConfigProvider::collect`]. Do not
    /// make the collection borrow the global pool.
    pub fn get(
        &self,
        source: &Path,
        contract_name: &str,
    ) -> Result<Vec<FunctionOverride>, InlineConfigError> {
        let (sender, receiver) = crossbeam_channel::bounded(1);
        let request = InlineConfigRequest {
            source: source.to_path_buf(),
            contract_name: contract_name.to_owned(),
            response_sender: sender,
        };

        self.request_sender
            .send(request)
            .expect("inline-config provider request channel should be open");

        receiver
            .recv()
            .expect("inline-config provider response channel should be open")
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

    /// Writes `SOURCE` to a temporary file and returns a root under the
    /// (namespaced, non-path) query identity `project/test.sol`, together with
    /// the handle keeping the file alive.
    fn root_with_source() -> (InlineConfigRoot, tempfile::NamedTempFile) {
        let mut file = tempfile::Builder::new()
            .suffix(".sol")
            .tempfile()
            .expect("temp file");
        file.write_all(SOURCE.as_bytes()).expect("write source");

        let root = InlineConfigRoot {
            source: PathBuf::from(SOURCE_NAME),
            path: file.path().to_path_buf(),
            version: Version::new(0, 8, 0),
        };
        (root, file)
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

        assert_overrides(&provider.get(Path::new(SOURCE_NAME), "MyTest").unwrap());
        // A source that was never collected reports no overrides.
        assert!(provider
            .get(Path::new("never.sol"), "MyTest")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn cached_collect_fails_on_missing_root_file() {
        let root = InlineConfigRoot {
            source: PathBuf::from(SOURCE_NAME),
            path: PathBuf::from("/nonexistent/test.sol"),
            version: Version::new(0, 8, 0),
        };

        let error = CachedInlineConfigProvider::collect(&[root], &ImportResolver::default())
            .expect_err("missing root file fails collection");
        assert!(matches!(
            error,
            InlineConfigCollectError::RootFileNotFound { .. }
        ));
    }

    #[test]
    fn shared_collects_then_serves_concurrent_queries() {
        let (root, _file) = root_with_source();
        let provider = SharedInlineConfigProvider::collect_in_background(
            vec![root],
            ImportResolver::default(),
        );

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let provider = provider.clone();
                thread::spawn(move || provider.get(Path::new(SOURCE_NAME), "MyTest").unwrap())
            })
            .collect();

        for handle in handles {
            assert_overrides(&handle.join().unwrap());
        }
    }

    #[test]
    fn shared_synchronous_collect_then_serves() {
        let (root, _file) = root_with_source();
        let provider = SharedInlineConfigProvider::collect(vec![root], ImportResolver::default())
            .expect("collect succeeds");

        assert_overrides(&provider.get(Path::new(SOURCE_NAME), "MyTest").unwrap());
    }

    #[test]
    fn shared_synchronous_collect_fails_eagerly_on_missing_root_file() {
        let root = InlineConfigRoot {
            source: PathBuf::from(SOURCE_NAME),
            path: PathBuf::from("/nonexistent/test.sol"),
            version: Version::new(0, 8, 0),
        };

        // Unlike `collect_in_background`, a collection failure surfaces
        // immediately rather than being deferred to the first query.
        let error = SharedInlineConfigProvider::collect(vec![root], ImportResolver::default())
            .expect_err("missing root file fails collection eagerly");
        assert!(matches!(
            error,
            InlineConfigCollectError::RootFileNotFound { .. }
        ));
    }
}
