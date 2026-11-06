#[derive(Debug, thiserror::Error)]
pub enum TestRunnerError {
    #[error("Failed to create executor: {0}")]
    ExecutorBuilderError(#[from] foundry_evm::executors::ExecutorBuilderError),
    #[error("Failed to make room for the fuzz failure directory: {0}")]
    LegacyFuzzFailureFile(#[from] edr_common::errors::FsPathError),
}
