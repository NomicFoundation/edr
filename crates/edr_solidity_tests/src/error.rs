#[derive(Debug, thiserror::Error)]
pub enum TestRunnerError {
    #[error("Failed to create executor: {0}")]
    ExecutorBuilderError(#[from] foundry_evm::executors::ExecutorBuilderError),
    /// A fuzz failure file written by an earlier EDR version could not be
    /// removed to make room for the fuzz failure directory.
    #[error(
        "Failed to remove the fuzz failure file left by an earlier EDR version, which blocks the fuzz failure directory: {0}. Delete the file or change `failurePersistDir`."
    )]
    LegacyFuzzFailureFile(#[source] edr_common::errors::FsPathError),
    /// A file that EDR did not write occupies the path of the fuzz failure
    /// directory.
    #[error(
        "{} blocks the fuzz failure directory and is not a fuzz failure file written by an earlier EDR version. Move the file or change `failurePersistDir`.",
        .0.display()
    )]
    UnknownFileAtFuzzFailureDir(std::path::PathBuf),
}
