use std::path::PathBuf;

use alloy_primitives::U256;

/// Contains for fuzz testing
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FuzzConfig {
    /// The number of test cases that must execute for each property test
    pub runs: u32,
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
}

impl Default for FuzzConfig {
    fn default() -> Self {
        FuzzConfig {
            runs: 256,
            max_test_rejects: 65536,
            seed: None,
            dictionary: FuzzDictionaryConfig::default(),
            gas_report_samples: 0,
            failure_persist_dir: None,
            failure_persist_file: "failures".into(),
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