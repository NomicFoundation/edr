use std::{
    fs,
    path::{Path, PathBuf},
};

const DEFAULT_REGISTRY_INDEX: &str = "registry+https://github.com/rust-lang/crates.io-index";
const DEFAULT_SPARSE_REGISTRY_INDEX: &str = "registry+sparse+https://index.crates.io/";

#[derive(Debug, Clone)]
pub struct Config {
    pub cooldown_minutes: u64,
    pub cache_dir: Option<PathBuf>,
    pub cache_ttl_seconds: u64,
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

    pub fn load(file_path: &Path) -> anyhow::Result<Self> {
        let file_config = CooldownFileConfig::load(file_path)?;
        log::info!("Cooldown config: {file_config:?}");
        let default = Config::default();
        let config = Config {
            cooldown_minutes: file_config.cooldown_minutes,
            cache_dir: file_config.cache_dir,
            cache_ttl_seconds: file_config
                .cache_ttl_seconds
                .unwrap_or(default.cache_ttl_seconds),
            ..default
        };
        Ok(config)
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
struct CooldownFileConfig {
    cooldown_minutes: u64,
    cache_dir: Option<PathBuf>,
    cache_ttl_seconds: Option<u64>,
}

impl CooldownFileConfig {
    fn load(file_path: &Path) -> anyhow::Result<Self> {
        let file_contents = fs::read_to_string(file_path)?;
        let cooldown_config: CooldownFileConfig = toml::from_str(&file_contents)?;
        Ok(cooldown_config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cooldown_minutes: 10080,   // 7 days
            cache_ttl_seconds: 86_400, // 1 day
            allowed_registries: default_allowed_registries(),
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

    mod cooldown_file_config {
        use std::io::Write;

        use tempfile::NamedTempFile;

        use super::*;

        #[test]
        fn load_respects_all_fields() {
            let mut file = NamedTempFile::new().unwrap();
            writeln!(
                file,
                "cooldown_minutes = 60\ncache_dir = \"/tmp/my-cache\"\ncache_ttl_seconds = 3600"
            )
            .unwrap();

            let config = Config::load(file.path()).unwrap();
            assert_eq!(config.cooldown_minutes, 60);
            assert_eq!(config.cache_dir, Some(PathBuf::from("/tmp/my-cache")));
            assert_eq!(config.cache_ttl_seconds, 3600);
        }

        #[test]
        fn load_uses_defaults_for_optional_fields() {
            let mut file = NamedTempFile::new().unwrap();
            writeln!(file, "cooldown_minutes = 120").unwrap();

            let config = Config::load(file.path()).unwrap();
            assert_eq!(config.cooldown_minutes, 120);
            assert_eq!(config.cache_dir, None);
            assert_eq!(config.cache_ttl_seconds, 86_400);
        }
    }
}
