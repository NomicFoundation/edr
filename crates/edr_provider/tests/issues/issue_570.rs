use std::str::FromStr as _;

use edr_eth::{spec::HardforkActivations, SpecId, B256};
use edr_provider::{
    hardhat_rpc_types::ForkConfig, test_utils::create_test_config_with_fork, time::CurrentTime,
    MethodInvocation, NoopLogger, Provider, ProviderRequest,
};
use edr_test_utils::env::get_alchemy_url;
use tokio::runtime;

// `eth_debugTraceTransaction` should return a helpful error message if there is
// a transaction in the block whose type is not supported.
// https://github.com/NomicFoundation/edr/issues/570
#[tokio::test(flavor = "multi_thread")]
async fn issue_570_error_message() -> anyhow::Result<()> {
    let logger = Box::new(NoopLogger);
    let subscriber = Box::new(|_event| {});

    let mut config = create_test_config_with_fork(Some(ForkConfig {
        json_rpc_url: get_alchemy_url().replace("eth-mainnet", "base-sepolia"),
        block_number: Some(13_560_400),
        http_headers: None,
    }));

    let chain_id = 84532;

    config
        .chains
        .insert(chain_id, HardforkActivations::with_spec_id(SpecId::CANCUN));

    // The default chain id set by Hardhat
    config.chain_id = chain_id;

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        CurrentTime,
    )?;

    let transaction_hash =
        B256::from_str("0xe565eb3bfd815efcc82bed1eef580117f9dc3d6896db42500572c8e789c5edd4")?;

    let result = provider.handle_request(ProviderRequest::Single(
        MethodInvocation::DebugTraceTransaction(transaction_hash, None),
    ));
    assert!(result
        .expect_err("should error")
        .to_string()
        .contains("unsupported type"));

    Ok(())
}

// `eth_debugTraceTransaction` should ignore transactions with unsupported types
// if a custom environment variable is set.
// https://github.com/NomicFoundation/edr/issues/570
#[tokio::test(flavor = "multi_thread")]
async fn issue_570_env_var() -> anyhow::Result<()> {
    let logger = Box::new(NoopLogger);
    let subscriber = Box::new(|_event| {});

    let mut config = create_test_config_with_fork(Some(ForkConfig {
        json_rpc_url: get_alchemy_url().replace("eth-mainnet", "base-sepolia"),
        block_number: Some(13_560_400),
        http_headers: None,
    }));

    let chain_id = 84532;

    config
        .chains
        .insert(chain_id, HardforkActivations::with_spec_id(SpecId::CANCUN));

    // The default chain id set by Hardhat
    config.chain_id = chain_id;

    std::env::set_var("__EDR_UNSAFE_SKIP_UNSUPPORTED_TRANSACTION_TYPES", "true");
    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        CurrentTime,
    );
    std::env::remove_var("__EDR_UNSAFE_SKIP_UNSUPPORTED_TRANSACTION_TYPES");
    let provider = provider?;

    let transaction_hash =
        B256::from_str("0xe565eb3bfd815efcc82bed1eef580117f9dc3d6896db42500572c8e789c5edd4")?;

    let result = provider.handle_request(ProviderRequest::Single(
        MethodInvocation::DebugTraceTransaction(transaction_hash, None),
    ))?;

    assert!(!result.traces.is_empty());

    Ok(())
}
