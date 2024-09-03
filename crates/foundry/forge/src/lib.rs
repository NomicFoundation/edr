#[macro_use]
extern crate tracing;

use foundry_config::{FuzzConfig, InvariantConfig};
use proptest::test_runner::{
    FailurePersistence, FileFailurePersistence, RngAlgorithm, TestRng, TestRunner,
};

pub mod coverage;

pub mod gas_report;

pub mod multi_runner;
pub use multi_runner::MultiContractRunner;

mod runner;
pub use runner::ContractRunner;

mod config;
pub use config::{SolidityTestRunnerConfig, SolidityTestRunnerConfigError};

pub mod result;

// TODO: remove
pub use foundry_common::traits::TestFilter;
pub use foundry_evm::*;

/// Metadata on how to run fuzz/invariant tests
#[derive(Clone, Debug, Default)]
pub struct TestOptions {
    /// The base "fuzz" test configuration
    pub fuzz: FuzzConfig,
    /// The base "invariant" test configuration
    pub invariant: InvariantConfig,
}

impl TestOptions {
    /// Returns a "fuzz" test runner instance.
    pub fn fuzz_runner(&self) -> TestRunner {
        let fuzz_config = self.fuzz_config().clone();
        let failure_persist_path = fuzz_config
            .failure_persist_dir
            .unwrap()
            .join(fuzz_config.failure_persist_file.unwrap())
            .into_os_string()
            .into_string()
            .unwrap();
        self.fuzzer_with_cases(
            fuzz_config.runs,
            Some(Box::new(FileFailurePersistence::Direct(
                failure_persist_path.leak(),
            ))),
        )
    }

    /// Returns an "invariant" test runner instance.
    pub fn invariant_runner(&self) -> TestRunner {
        let invariant = self.invariant_config();
        self.fuzzer_with_cases(invariant.runs, None)
    }

    /// Returns a "fuzz" configuration setup.
    pub fn fuzz_config(&self) -> &FuzzConfig {
        &self.fuzz
    }

    /// Returns an "invariant" configuration setup.
    pub fn invariant_config(&self) -> &InvariantConfig {
        &self.invariant
    }

    pub fn fuzzer_with_cases(
        &self,
        cases: u32,
        file_failure_persistence: Option<Box<dyn FailurePersistence>>,
    ) -> TestRunner {
        let config = proptest::test_runner::Config {
            failure_persistence: file_failure_persistence,
            cases,
            max_global_rejects: self.fuzz.max_test_rejects,
            ..Default::default()
        };

        if let Some(seed) = &self.fuzz.seed {
            trace!(target: "forge::test", %seed, "building deterministic fuzzer");
            let rng = TestRng::from_seed(RngAlgorithm::ChaCha, &seed.to_be_bytes::<32>());
            TestRunner::new_with_rng(config, rng)
        } else {
            trace!(target: "forge::test", "building stochastic fuzzer");
            TestRunner::new(config)
        }
    }
}
