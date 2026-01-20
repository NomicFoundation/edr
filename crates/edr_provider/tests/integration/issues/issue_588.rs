//! Allow forking blocks with future timestamps.
//!
//! See <https://github.com/NomicFoundation/edr/issues/588>

use std::sync::Arc;

use edr_chain_l1::L1ChainSpec;
use edr_primitives::HashMap;
use edr_provider::{
    test_utils::{create_test_config_with, BasicProviderConfig},
    time::MockTime,
    ForkConfig, NoopLogger, Provider,
};
use edr_solidity::contract_decoder::ContractDecoder;
use edr_test_utils::env::json_rpc_url_provider;
use tokio::runtime;

#[tokio::test(flavor = "multi_thread")]
async fn issue_588() -> anyhow::Result<()> {
    let logger = Box::new(NoopLogger::<L1ChainSpec, Arc<MockTime>>::default());
    let subscriber = Box::new(|_event| {});

    let early_mainnet_fork =
        create_test_config_with(BasicProviderConfig::fork_with_accounts(ForkConfig {
            block_number: Some(2_675_000),
            cache_dir: edr_defaults::CACHE_DIR.into(),
            chain_overrides: HashMap::default(),
            http_headers: None,
            url: json_rpc_url_provider::ethereum_mainnet(),
        }));

    let current_time_is_1970 = Arc::new(MockTime::with_seconds(0));

    let _forking_succeeds = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        early_mainnet_fork,
        Arc::<ContractDecoder>::default(),
        current_time_is_1970,
    )?;

    Ok(())
}
