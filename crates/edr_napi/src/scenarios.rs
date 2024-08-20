use std::time::{SystemTime, UNIX_EPOCH};

use edr_eth::chain_spec::L1ChainSpec;
use edr_evm::chain_spec::ChainSpec;
use edr_provider::ProviderRequest;
use napi::tokio::{fs::File, io::AsyncWriteExt, sync::Mutex};
use rand::{distributions::Alphanumeric, Rng};
use serde::{de::DeserializeOwned, Serialize};

const SCENARIO_FILE_PREFIX: &str = "EDR_SCENARIO_PREFIX";

#[derive(Serialize)]
#[serde(bound = "ChainSpecT::Hardfork: Serialize")]
struct ScenarioConfig<'a, ChainSpecT: ChainSpec> {
    chain_type: &'a str,
    provider_config: &'a edr_provider::ProviderConfig<ChainSpecT>,
    logger_enabled: bool,
}

pub(crate) async fn scenario_file<ChainSpecT: ChainSpec<Hardfork: DeserializeOwned + Serialize>>(
    provider_config: &edr_provider::ProviderConfig<ChainSpecT>,
    logger_enabled: bool,
) -> napi::Result<Option<Mutex<File>>> {
    if let Ok(scenario_prefix) = std::env::var(SCENARIO_FILE_PREFIX) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();
        let suffix = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(4)
            .map(char::from)
            .collect::<String>();

        let mut scenario_file =
            File::create(format!("{scenario_prefix}_{timestamp}_{suffix}.json")).await?;

        let config = ScenarioConfig {
            chain_type,
            provider_config,
            logger_enabled,
        };
        let mut line = serde_json::to_string(&config)?;
        line.push('\n');
        scenario_file.write_all(line.as_bytes()).await?;

        Ok(Some(Mutex::new(scenario_file)))
    } else {
        Ok(None)
    }
}

pub(crate) async fn write_request(
    scenario_file: &Mutex<File>,
    request: &ProviderRequest<L1ChainSpec>,
) -> napi::Result<()> {
    let mut line = serde_json::to_string(request)?;
    line.push('\n');
    {
        let mut scenario_file = scenario_file.lock().await;
        scenario_file.write_all(line.as_bytes()).await?;
    }
    Ok(())
}
