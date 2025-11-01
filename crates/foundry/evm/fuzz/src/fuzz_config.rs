use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};
use alloy_primitives::U256;
use proptest::test_runner::{FailurePersistence, FileFailurePersistence};

static FAILURE_PATHS: OnceLock<RwLock<HashSet<&'static str>>> = OnceLock::new();

/// Contains for fuzz testing
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FuzzConfig {
    /// The number of test cases that must execute for each property test
    pub runs: u32,
    /// Fails the fuzzed test if a revert occurs.
    pub fail_on_revert: bool,
    /// The maximum number of test case rejections allowed by proptest, to be
    /// encountered during usage of `vm.assume` cheatcode. This will be used
    /// to set the `max_global_rejects` value in proptest test runner config.
    /// `max_local_rejects` option isn't exposed here since we're not using
    /// `prop_filter`.
    pub max_test_rejects: u32,
    /// Optional seed for the fuzzing RNG algorithm
    pub seed: Option<U256>,
    /// The fuzz dictionary configuration
    pub dictionary: FuzzDictionaryConfig,
    /// Number of runs to execute and include in the gas report.
    pub gas_report_samples: u32,
    /// Path where fuzz failures are recorded and replayed.
    pub failure_persist_dir: Option<PathBuf>,
    /// Name of the file to record fuzz failures, defaults to `failures`.
    pub failure_persist_file: String,
    /// show `console.log` in fuzz test, defaults to `false`
    pub show_logs: bool,
    /// Optional timeout (in seconds) for each property test
    pub timeout: Option<u32>,
}

impl Default for FuzzConfig {
    fn default() -> Self {
        FuzzConfig {
            runs: 256,
            fail_on_revert: true,
            max_test_rejects: 65536,
            seed: None,
            dictionary: FuzzDictionaryConfig::default(),
            gas_report_samples: 0,
            failure_persist_dir: None,
            failure_persist_file: "failures".to_string(),
            show_logs: false,
            timeout: None,
        }
    }
}

impl FuzzConfig {
    /// Creates fuzz configuration to write failures in
    /// `{PROJECT_ROOT}/cache/fuzz` dir.
    pub fn new(cache_dir: PathBuf) -> Self {
        FuzzConfig {
            failure_persist_dir: Some(cache_dir),
            ..FuzzConfig::default()
        }
    }

    /// Returns file failure persistance for the fuzzer.
    pub fn file_failure_persistence(&self) -> Option<Box<dyn FailurePersistence>> {
        if let Some(failure_persist_dir) = self.failure_persist_dir.as_ref() {
            let failure_persist_path = failure_persist_dir
                .join(&self.failure_persist_file)
                .into_os_string()
                .into_string()
                .expect("path should be valid UTF-8");

            // HACK: We need to leak the path as
            // `proptest::test_runner::FileFailurePersistence` requires a
            // `&'static str`. We mitigate this by making sure that one particular path
            // is only leaked once.
            let failure_paths = FAILURE_PATHS.get_or_init(RwLock::default);
            // Need to be in a block to ensure that the read lock is dropped before we try
            // to insert.
            {
                let failure_paths_guard = failure_paths.read().expect("lock is not poisoned");
                if let Some(static_path) = failure_paths_guard.get(&*failure_persist_path) {
                    return Some(Box::new(FileFailurePersistence::Direct(static_path)))
                }
            }
            // Write block
            {
                let mut failure_paths_guard = failure_paths.write().expect("lock is not poisoned");
                failure_paths_guard.insert(failure_persist_path.clone().leak());
                let static_path = failure_paths_guard
                    .get(&*failure_persist_path)
                    .expect("must exist since we just inserted it");

                Some(Box::new(FileFailurePersistence::Direct(static_path)))
            }
        } else {
            None
        }
    }
}

/// Contains for fuzz testing
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FuzzDictionaryConfig {
    /// The weight of the dictionary
    pub dictionary_weight: u32,
    /// The flag indicating whether to include values from storage
    pub include_storage: bool,
    /// The flag indicating whether to include push bytes values
    pub include_push_bytes: bool,
    /// How many addresses to record at most.
    /// Once the fuzzer exceeds this limit, it will start evicting random
    /// entries
    ///
    /// This limit is put in place to prevent memory blowup.
    pub max_fuzz_dictionary_addresses: usize,
    /// How many values to record at most.
    /// Once the fuzzer exceeds this limit, it will start evicting random
    /// entries
    pub max_fuzz_dictionary_values: usize,
}

impl Default for FuzzDictionaryConfig {
    fn default() -> Self {
        FuzzDictionaryConfig {
            dictionary_weight: 40,
            include_storage: true,
            include_push_bytes: true,
            // limit this to 300MB
            max_fuzz_dictionary_addresses: (300 * 1024 * 1024) / 20,
            // limit this to 200MB
            max_fuzz_dictionary_values: (200 * 1024 * 1024) / 32,
        }
    }
}
