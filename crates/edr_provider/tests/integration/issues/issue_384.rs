use std::sync::Arc;

use edr_eth::{HashMap, l1::L1ChainSpec};
use edr_provider::{
    ForkConfig, MethodInvocation, NoopLogger, Provider, ProviderRequest,
    test_utils::create_test_config_with_fork, time::CurrentTime,
};
use edr_solidity::contract_decoder::ContractDecoder;
use edr_test_utils::env::get_infura_url;
use tokio::runtime;

#[tokio::test(flavor = "multi_thread")]
async fn avalanche_chain_mine_local_block() -> anyhow::Result<()> {
    const BLOCK_NUMBER: u64 = 22_587_773;

    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});

    let config = create_test_config_with_fork(Some(ForkConfig {
        block_number: Some(BLOCK_NUMBER),
        cache_dir: edr_defaults::CACHE_DIR.into(),
        chain_overrides: HashMap::new(),
        http_headers: None,
        url: get_infura_url().replace("mainnet", "avalanche-mainnet"),
    }));

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?;

    provider.handle_request(ProviderRequest::Single(MethodInvocation::EvmMine(None)))?;

    Ok(())
}
