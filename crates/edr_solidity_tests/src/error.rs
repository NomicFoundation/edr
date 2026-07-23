#[derive(Debug, thiserror::Error)]
pub enum TestRunnerError {
    #[error("Failed to create executor: {0}")]
    ExecutorBuilderError(#[from] foundry_evm::executors::ExecutorBuilderError),
}
