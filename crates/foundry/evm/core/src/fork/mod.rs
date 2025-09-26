use std::path::PathBuf;

use super::opts::EvmOpts;

mod init;
use alloy_chains::Chain;
pub use init::{configure_env, environment};

pub mod database;

mod multi;
pub mod provider;

pub use multi::{ForkId, MultiFork, MultiForkHandler};

use crate::evm_context::{BlockEnvTr, EvmEnv, HardforkTr, TransactionEnvTr};

/// Represents a _fork_ of a remote chain whose data is available only via the `url` endpoint.
#[derive(Clone, Debug)]
pub struct CreateFork<BlockT, TxT, HardforkT> {
    /// Optional RPC cache path. If this is none, then no rpc calls will be
    /// cached, otherwise data is cached to `<rpc_cache_path>/<chain id>/<block
    /// number>`.
    pub rpc_cache_path: Option<PathBuf>,
    /// The URL to a node for fetching remote state
    pub url: String,
    /// The env to create this fork, main purpose is to provide some metadata for the fork
    pub env: EvmEnv<BlockT, TxT, HardforkT>,
    /// All env settings as configured by the user
    pub evm_opts: EvmOpts<HardforkT>,
}

impl<BlockT: BlockEnvTr, TxT: TransactionEnvTr, HardforkT: HardforkTr>
    CreateFork<BlockT, TxT, HardforkT>
{
    /// Returns the path to the cache dir of the `block` on the `chain`
    /// based on the configured rpc cache path.
    pub fn block_cache_dir(&self, chain_id: impl Into<Chain>, block: u64) -> Option<PathBuf> {
        self.rpc_cache_path.as_ref().map(|rpc_cache_path| {
            rpc_cache_path.join(chain_id.into().to_string()).join(block.to_string())
        })
    }
}
