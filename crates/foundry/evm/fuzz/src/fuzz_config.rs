use std::path::PathBuf;

use alloy_primitives::U256;

/// Contains for fuzz testing
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FuzzConfig {
    /// The number of test cases that must execute for each property test
    pub runs: u32,
    /// Fails the fuzzed test if a revert occurs.
    pub fail_on_revert: bool,
    /// The maximum number of test case rejections allowed, to be encountered
    /// during usage of `vm.assume` cheatcode. Set to `0` to disable the limit.
    pub max_test_rejects: u32,
    /// Optional seed for the fuzzing RNG algorithm
    pub seed: Option<U256>,
    /// The fuzz dictionary configuration
    pub dictionary: FuzzDictionaryConfig,
    /// Number of runs to execute and include in the gas report.
    pub gas_report_samples: u32,
    /// Path where fuzz failures are recorded and replayed.
    pub failure_persist_dir: Option<PathBuf>,
    /// Name of the directory under [`Self::failure_persist_dir`] in which fuzz
    /// failures are recorded, defaults to `failures`.
    pub failure_persist_file: String,
    /// show `console.log` in fuzz test, defaults to `false`
    pub show_logs: bool,
    /// Optional timeout (in seconds) for each property test
    pub timeout: Option<u32>,
    /// Number of parallel workers used to run each fuzz test. `None` picks
    /// the number of available threads, bounded by the number of runs.
    pub workers: Option<u32>,
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
            workers: None,
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

    /// Returns the root directory under which fuzz failures are persisted,
    /// `<failure_persist_dir>/<failure_persist_file>`, if failure persistence
    /// is enabled.
    pub fn failure_persist_root(&self) -> Option<PathBuf> {
        self.failure_persist_dir
            .as_ref()
            .map(|failure_persist_dir| failure_persist_dir.join(&self.failure_persist_file))
    }

    /// Returns the failure directory and failure file of the given fuzz test,
    /// if failure persistence is enabled.
    ///
    /// Failures are persisted as
    /// `<failure_persist_dir>/<failure_persist_file>/<contract name>/<test
    /// name>`.
    pub fn failure_paths(
        &self,
        contract_name: &str,
        test_name: &str,
    ) -> Option<(PathBuf, PathBuf)> {
        self.failure_persist_root().map(|root| {
            let dir = root.join(
                contract_name
                    .split(':')
                    .next_back()
                    .expect("split always yields at least one element"),
            );
            let file = dir.join(test_name);
            (dir, file)
        })
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
