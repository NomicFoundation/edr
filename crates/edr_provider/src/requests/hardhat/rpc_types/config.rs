use std::path::PathBuf;

use edr_eth::{ChainId, HashMap};
use edr_evm::hardfork::ChainOverride;

use crate::config;

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ResetProviderConfig {
    pub forking: Option<ResetForkConfig>,
}

/// Configuration for resetting the [`crate::ForkConfig`] of a provider.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetForkConfig {
    pub json_rpc_url: String,
    pub block_number: Option<u64>,
    pub http_headers: Option<std::collections::HashMap<String, String>>,
}

impl ResetForkConfig {
    /// Resolves the `ResetForkConfig` into a `ForkConfig`.
    ///
    /// If no `cache_dir` is provided, it will default to the value of
    /// `edr_defaults::CACHE_DIR`.
    pub fn resolve<HardforkT>(
        self,
        cache_dir: Option<PathBuf>,
        chain_overrides: HashMap<ChainId, ChainOverride<HardforkT>>,
    ) -> config::Fork<HardforkT> {
        config::Fork {
            block_number: self.block_number,
            cache_dir: cache_dir.unwrap_or(edr_defaults::CACHE_DIR.into()),
            chain_overrides,
            http_headers: self.http_headers,
            url: self.json_rpc_url,
        }
    }
}
