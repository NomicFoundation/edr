use std::{collections::HashMap, path::PathBuf};

use crate::config;

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ResetProviderConfig {
    pub forking: Option<ResetForkConfig>,
}

/// Configuration for resetting the [`ForkConfig`] of a provider.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetForkConfig {
    pub block_number: Option<u64>,
    pub http_headers: Option<HashMap<String, String>>,
    pub url: String,
}

impl ResetForkConfig {
    /// Resolves the `ResetForkConfig` into a `ForkConfig`.
    ///
    /// If no `cache_dir` is provided, it will default to the value of
    /// `edr_defaults::CACHE_DIR`.
    pub fn resolve(self, cache_dir: Option<PathBuf>) -> config::Fork {
        config::Fork {
            block_number: self.block_number,
            cache_dir: cache_dir.unwrap_or(edr_defaults::CACHE_DIR.into()),
            http_headers: self.http_headers,
            url: self.url,
        }
    }
}
