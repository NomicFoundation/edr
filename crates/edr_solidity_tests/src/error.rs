use edr_solidity_collector_eip712::collector::Eip712CollectError;

#[derive(Debug, thiserror::Error)]
pub enum TestRunnerError {
    #[error("Failed to create executor: {0}")]
    ExecutorBuilderError(#[from] foundry_evm::executors::ExecutorBuilderError),
    #[error(transparent)]
    Eip712TypeCollectionError(#[from] Eip712CollectError),
}
