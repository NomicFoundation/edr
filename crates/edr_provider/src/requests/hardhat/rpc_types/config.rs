use std::{collections::HashMap, path::PathBuf};

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ResetProviderConfig {
    pub forking: Option<ForkConfig>,
}

/// Configuration for forking a blockchain
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkConfig {
    pub block_number: Option<u64>,
    pub cache_dir: PathBuf,
    pub http_headers: Option<HashMap<String, String>>,
    pub url: String,
}
