#[derive(Debug, thiserror::Error)]
pub enum TestRunnerError {
    #[error("Failed to create executor: {0}")]
    ExecutorBuilderError(#[from] foundry_evm::executors::ExecutorBuilderError),
    /// One or more of the selected suites' sources could not be collected.
    /// Carried as structured problems so consumers can locate each one.
    #[error("Could not collect from the test sources:\n{0}")]
    InlineConfig(#[from] crate::inline_config::error::InlineConfigErrors),
}
