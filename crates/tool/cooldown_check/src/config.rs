use std::{fs, path::PathBuf};

use crate::{
    allowlist::Allowlist,
    workspace::{config_file_path, COOLDOWN_FILE_CONFIG},
};

const DEFAULT_REGISTRY_INDEX: &str = "registry+https://github.com/rust-lang/crates.io-index";
const DEFAULT_SPARSE_REGISTRY_INDEX: &str = "registry+sparse+https://index.crates.io/";

#[derive(Debug, Clone)]
pub struct Config {
    pub cooldown_minutes: u64,
    pub ttl_seconds: u64,
    // pub allowlist_path: Option<PathBuf>,
    pub cache_dir: Option<PathBuf>,
    pub offline_ok: bool,
    pub http_retries: u32,
    pub registry_api: String,
    pub allowed_registries: Vec<String>,
    pub allowlist: Allowlist,
}

impl Config {
    pub fn is_registry_allowed(&self, source: &str) -> bool {
        self.allowed_registries
            .iter()
            .any(|allowed| allowed == source)
    }

    pub fn load() -> anyhow::Result<Self> {
        let cooldown_config = CooldownFileConfig::load()?;
        let allowlist = Allowlist::load()?;
        log::debug!("cooldown config: {cooldown_config:?}");
        log::debug!("allowlist: {allowlist:?}");
        let config = Config {
            cooldown_minutes: cooldown_config.cooldown_minutes,
            allowlist,
            ..Config::default()
        };
        Ok(config)
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
struct CooldownFileConfig {
    cooldown_minutes: u64,
}

impl CooldownFileConfig {
    fn load() -> anyhow::Result<Self> {
        let file_contents = fs::read_to_string(config_file_path(COOLDOWN_FILE_CONFIG)?)?;
        let cooldown_config: CooldownFileConfig = toml::from_str(&file_contents)?;
        Ok(cooldown_config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cooldown_minutes: 10080, // 7 days
            ttl_seconds: 86_400,     // ttl of cache entries
            allowed_registries: default_allowed_registries(), // TODO: configurable by file?
            cache_dir: None,
            http_retries: 2,
            offline_ok: false,
            registry_api: "https://crates.io/api/v1/".to_string(),
            allowlist: Allowlist::default(),
        }
    }
}

fn default_allowed_registries() -> Vec<String> {
    vec![
        DEFAULT_REGISTRY_INDEX.to_string(),
        DEFAULT_SPARSE_REGISTRY_INDEX.to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_allowed_registries_include_sparse_and_git() {
        let config = Config::default();
        assert_eq!(config.allowed_registries, default_allowed_registries());
    }
}
