use std::sync::Arc;

use edr_chain_l1::L1ChainSpec;
use edr_primitives::HashMap;
use edr_provider::{
    test_utils::{create_test_config_with, MinimalProviderConfig},
    time::CurrentTime,
    ForkConfig, MethodInvocation, NoopLogger, Provider, ProviderRequest,
};
use edr_solidity::contract_decoder::ContractDecoder;
use edr_test_utils::env::json_rpc_url_provider;
use parking_lot::RwLock;
use tokio::runtime;

#[tokio::test(flavor = "multi_thread")]
async fn avalanche_chain_mine_local_block() -> anyhow::Result<()> {
    const BLOCK_NUMBER: u64 = 22_587_773;

    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});

    let config = create_test_config_with(MinimalProviderConfig::fork_with_accounts(ForkConfig {
        block_number: Some(BLOCK_NUMBER),
        cache_dir: edr_defaults::CACHE_DIR.into(),
        chain_overrides: HashMap::default(),
        http_headers: None,
        url: json_rpc_url_provider::avalanche_mainnet(),
    }));

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::new(RwLock::<ContractDecoder>::default()),
        CurrentTime,
    )?;

    provider.handle_request(ProviderRequest::with_single(MethodInvocation::EvmMine(
        None,
    )))?;

    Ok(())
}
