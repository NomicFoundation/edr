use std::collections::BTreeMap;

use edr_artifact::ArtifactId;
use edr_decoder_revert::RevertDecoder;
use edr_primitives::Bytes;
use edr_solidity_tests::{contracts::ContractsByArtifact, multi_runner::TestContract};
use napi::tokio;

use crate::solidity::{
    config::{TestRunnerConfig, TracingConfigWithBuffers},
    SyncTestRunner,
};

pub trait SyncTestRunnerFactory: Send + Sync {
    /// Creates `SyncTestRunner` instance
    #[allow(clippy::too_many_arguments)]
    fn create_test_runner(
        &self,
        runtime: tokio::runtime::Handle,
        config: TestRunnerConfig,
        contracts: BTreeMap<ArtifactId, TestContract>,
        known_contracts: ContractsByArtifact,
        libs_to_deploy: Vec<Bytes>,
        revert_decoder: RevertDecoder,
        tracing_config: TracingConfigWithBuffers,
    ) -> napi::Result<Box<dyn SyncTestRunner>>;
}
