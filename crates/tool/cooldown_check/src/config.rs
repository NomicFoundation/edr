use std::{fs, path::PathBuf};

const DEFAULT_REGISTRY_INDEX: &str = "registry+https://github.com/rust-lang/crates.io-index";
const DEFAULT_SPARSE_REGISTRY_INDEX: &str = "registry+sparse+https://index.crates.io/";

#[derive(Debug, Clone)]
pub struct Config {
    pub cooldown_minutes: u64,
    pub ttl_seconds: u64,
    pub cache_dir: Option<PathBuf>,
    pub http_retries: u32,
    pub registry_api: String,
    pub allowed_registries: Vec<String>,
}

impl Config {
    pub fn is_registry_allowed(&self, source: &str) -> bool {
        self.allowed_registries
            .iter()
            .any(|allowed| allowed == source)
    }

    pub fn load(file_path: PathBuf) -> anyhow::Result<Self> {
        let cooldown_config = CooldownFileConfig::load(file_path)?;
        log::debug!("cooldown config: {cooldown_config:?}");
        let config = Config {
            cooldown_minutes: cooldown_config.cooldown_minutes,
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
    fn load(file_path: PathBuf) -> anyhow::Result<Self> {
        let file_contents = fs::read_to_string(file_path)?;
        let cooldown_config: CooldownFileConfig = toml::from_str(&file_contents)?;
        Ok(cooldown_config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cooldown_minutes: 10080,                          // 7 days
            ttl_seconds: 86_400,                              // ttl of cache entries
            allowed_registries: default_allowed_registries(), // TODO: configurable by file?
            cache_dir: None,
            http_retries: 2,
            registry_api: "https://crates.io/api/v1/".to_string(),
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
