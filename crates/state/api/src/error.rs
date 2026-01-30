use edr_primitives::B256;
use edr_rpc_client::RpcClientError;

/// Combinatorial error for the state API
#[derive(Debug, thiserror::Error)]
pub enum StateError {
    /// No checkpoints to revert
    #[error("No checkpoints to revert.")]
    CannotRevert,
    /// Contract with specified code hash does not exist
    #[error("Contract with code hash `{0}` does not exist.")]
    InvalidCodeHash(B256),
    /// Specified state root does not exist
    #[error("State root `{state_root:?}` does not exist (fork: {is_fork}).")]
    InvalidStateRoot {
        /// Requested state root
        state_root: B256,
        /// Whether the state root was intended for a fork
        is_fork: bool,
    },
    /// Error from the underlying RPC client
    #[error(transparent)]
    Remote(#[from] RpcClientError),
    /// Unsupported
    #[error("The action `{action}` is unsupported. {}", details.as_ref().unwrap_or(&"".into()))]
    Unsupported {
        action: String,
        details: Option<String>,
    },
}
