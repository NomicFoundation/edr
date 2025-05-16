use std::{str::FromStr as _, sync::Arc};

use edr_eth::{B256, HashMap, l1::L1ChainSpec};
use edr_provider::{
    ForkConfig, MethodInvocation, NoopLogger, Provider, ProviderRequest,
    test_utils::create_test_config_with_fork, time::CurrentTime,
};
use edr_solidity::contract_decoder::ContractDecoder;
use edr_test_utils::env::get_alchemy_url;
use tokio::runtime;

// https://github.com/NomicFoundation/edr/issues/533
#[tokio::test(flavor = "multi_thread")]
async fn issue_533() -> anyhow::Result<()> {
    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});

    let mut config = create_test_config_with_fork(Some(ForkConfig {
        block_number: Some(20_384_300),
        cache_dir: edr_defaults::CACHE_DIR.into(),
        chain_overrides: HashMap::new(),
        http_headers: None,
        url: get_alchemy_url(),
    }));

    // The default chain id set by Hardhat
    config.chain_id = 31337;

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?;

    let transaction_hash =
        B256::from_str("0x0537316f37627655b7fe5e50e23f71cd835b377d1cde4226443c94723d036e32")?;

    let result = provider.handle_request(ProviderRequest::Single(
        MethodInvocation::DebugTraceTransaction(transaction_hash, None),
    ))?;

    assert!(!result.traces.is_empty());

    Ok(())
}
