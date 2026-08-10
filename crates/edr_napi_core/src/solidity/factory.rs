use std::collections::BTreeMap;

use edr_artifact::ArtifactId;
use edr_decoder_revert::RevertDecoder;
use edr_primitives::Bytes;
use edr_solidity_tests::{
    contracts::ContractsByArtifact, inline_config::error::InlineConfigErrors,
    multi_runner::TestContract,
};
use napi::tokio;

use crate::solidity::{
    config::{TestRunnerConfig, TracingConfigWithBuffers},
    SyncTestRunner,
};

/// The reason creating a Solidity test runner failed.
pub enum CreateTestRunnerError {
    /// One or more test sources carry ill-formed inline configuration. Carried
    /// as the structured, locatable problems so the caller can surface them to
    /// the JS side rather than a flat string.
    InvalidInlineConfig(InlineConfigErrors),
    /// Any other failure, already rendered for the JS side.
    Failed(napi::Error),
}

impl From<napi::Error> for CreateTestRunnerError {
    fn from(error: napi::Error) -> Self {
        Self::Failed(error)
    }
}

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
    ) -> Result<Box<dyn SyncTestRunner>, CreateTestRunnerError>;
}
