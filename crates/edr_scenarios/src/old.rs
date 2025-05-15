use serde::{Deserialize, Serialize};

/// Helper struct to convert an old scenario format to the new one.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScenarioConfig {
    chain_type: Option<String>,
    logger_enabled: bool,
    provider_config: ScenarioProviderConfig,
}

impl From<ScenarioConfig> for super::ScenarioConfig {
    fn from(value: ScenarioConfig) -> Self {
        Self {
            chain_type: value.chain_type,
            logger_enabled: value.logger_enabled,
            provider_config: value.provider_config,
        }
    }
}

/// Placeholder for the actual old scenario provider config.
/// This should be replaced with the actual old scenario provider config
pub type ScenarioProviderConfig = super::ScenarioProviderConfig;
