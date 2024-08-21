use std::{convert::Infallible, str::FromStr as _};

use edr_eth::{spec::HardforkActivations, SpecId, B256};
use edr_provider::{
    hardhat_rpc_types::ForkConfig, test_utils::create_test_config_with_fork, time::CurrentTime,
    MethodInvocation, NoopLogger, Provider, ProviderError, ProviderRequest,
};
use edr_test_utils::env::get_alchemy_url;
use serial_test::serial;
use tokio::runtime;

// SAFETY: tests that modify the environment should be run serially.

fn get_provider() -> anyhow::Result<Provider<Infallible>> {
    // Base Sepolia Testnet
    const CHAIN_ID: u64 = 84532;
    const BLOCK_NUMBER: u64 = 13_560_400;

    let logger = Box::new(NoopLogger);
    let subscriber = Box::new(|_event| {});

    let mut config = create_test_config_with_fork(Some(ForkConfig {
        json_rpc_url: get_alchemy_url().replace("eth-mainnet", "base-sepolia"),
        block_number: Some(BLOCK_NUMBER),
        http_headers: None,
    }));

    config
        .chains
        .insert(CHAIN_ID, HardforkActivations::with_spec_id(SpecId::CANCUN));

    config.chain_id = CHAIN_ID;

    Ok(Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        CurrentTime,
    )?)
}

// `eth_debugTraceTransaction` should return a helpful error message if there is
// a transaction in the block whose type is not supported.
// https://github.com/NomicFoundation/edr/issues/570
#[serial]
#[tokio::test(flavor = "multi_thread")]
async fn issue_570_error_message() -> anyhow::Result<()> {
    let provider = get_provider()?;

    let transaction_hash =
        B256::from_str("0xe565eb3bfd815efcc82bed1eef580117f9dc3d6896db42500572c8e789c5edd4")?;

    let result = provider.handle_request(ProviderRequest::Single(
        MethodInvocation::DebugTraceTransaction(transaction_hash, None),
    ));

    assert!(matches!(
        result,
        Err(ProviderError::UnsupportedTransactionTypeInDebugTrace {
            requested_transaction_hash,
            unsupported_transaction_hash,
            ..
        }) if requested_transaction_hash == transaction_hash && unsupported_transaction_hash != transaction_hash
    ));

    Ok(())
}

// `eth_debugTraceTransaction` should ignore transactions with unsupported types
// if a custom environment variable is set.
// https://github.com/NomicFoundation/edr/issues/570
#[serial]
#[tokio::test(flavor = "multi_thread")]
async fn issue_570_env_var() -> anyhow::Result<()> {
    std::env::set_var("__EDR_UNSAFE_SKIP_UNSUPPORTED_TRANSACTION_TYPES", "true");
    let provider = get_provider();
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

// `eth_debugTraceTransaction` should return a helpful error message if tracing
// is requested for a transaction with an unsupported type. https://github.com/NomicFoundation/edr/issues/570
#[serial]
#[tokio::test(flavor = "multi_thread")]
async fn issue_570_unsupported_requested() -> anyhow::Result<()> {
    std::env::set_var("__EDR_UNSAFE_SKIP_UNSUPPORTED_TRANSACTION_TYPES", "true");
    let provider = get_provider();
    std::env::remove_var("__EDR_UNSAFE_SKIP_UNSUPPORTED_TRANSACTION_TYPES");
    let provider = provider?;

    let transaction_hash =
        B256::from_str("0xa9d8bf76337ac4a72a4085d5fd6456f6950b6b95d9d4aa198707a649268ef91c")?;

    let result = provider.handle_request(ProviderRequest::Single(
        MethodInvocation::DebugTraceTransaction(transaction_hash, None),
    ));

    assert!(matches!(
        result,
        Err(ProviderError::UnsupportedTransactionTypeForDebugTrace {
            transaction_hash: error_transaction_hash,
            ..
        }) if error_transaction_hash == transaction_hash
    ));

    Ok(())
}
