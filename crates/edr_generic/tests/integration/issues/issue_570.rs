#![cfg(feature = "test-remote")]
use std::str::FromStr as _;

use edr_eth::{l1, B256};
use edr_evm::hardfork::{self, ChainOverride};
use edr_generic::GenericChainSpec;
use edr_provider::{MethodInvocation, Provider, ProviderError, ProviderRequest};
use edr_test_utils::env::get_alchemy_url;
use serial_test::serial;

use crate::integration::helpers::get_chain_fork_provider;

// SAFETY: tests that modify the environment should be run serially.

fn get_provider() -> anyhow::Result<Provider<GenericChainSpec>> {
    // Base Sepolia Testnet
    const CHAIN_ID: u64 = 84532;
    const BLOCK_NUMBER: u64 = 13_560_400;

    let chain_override = ChainOverride {
        name: "Base Sepolia".to_owned(),
        hardfork_activation_overrides: Some(hardfork::Activations::with_spec_id(
            l1::SpecId::CANCUN,
        )),
        base_fee_params: None,
    };
    let url = get_alchemy_url().replace("eth-mainnet", "base-sepolia");

    get_chain_fork_provider::<GenericChainSpec>(CHAIN_ID, BLOCK_NUMBER, chain_override, url)
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

    let result = provider.handle_request(ProviderRequest::with_single(
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
    // THIS CALL IS UNSAFE AND MIGHT LEAD TO UNDEFINED BEHAVIOR. WE DEEM THE RISK
    // ACCEPTABLE FOR TESTING PURPOSES ONLY.
    unsafe { std::env::set_var("__EDR_UNSAFE_SKIP_UNSUPPORTED_TRANSACTION_TYPES", "true") };

    let provider = get_provider();

    // THIS CALL IS UNSAFE AND MIGHT LEAD TO UNDEFINED BEHAVIOR. WE DEEM THE RISK
    // ACCEPTABLE FOR TESTING PURPOSES ONLY.
    unsafe { std::env::remove_var("__EDR_UNSAFE_SKIP_UNSUPPORTED_TRANSACTION_TYPES") };

    let provider = provider?;

    let transaction_hash =
        B256::from_str("0xe565eb3bfd815efcc82bed1eef580117f9dc3d6896db42500572c8e789c5edd4")?;

    let result = provider.handle_request(ProviderRequest::with_single(
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
    // THIS CALL IS UNSAFE AND MIGHT LEAD TO UNDEFINED BEHAVIOR. WE DEEM THE RISK
    // ACCEPTABLE FOR TESTING PURPOSES ONLY.
    unsafe { std::env::set_var("__EDR_UNSAFE_SKIP_UNSUPPORTED_TRANSACTION_TYPES", "true") };

    let provider = get_provider();

    // THIS CALL IS UNSAFE AND MIGHT LEAD TO UNDEFINED BEHAVIOR. WE DEEM THE RISK
    // ACCEPTABLE FOR TESTING PURPOSES ONLY.
    unsafe { std::env::remove_var("__EDR_UNSAFE_SKIP_UNSUPPORTED_TRANSACTION_TYPES") };

    let provider = provider?;

    let transaction_hash =
        B256::from_str("0xa9d8bf76337ac4a72a4085d5fd6456f6950b6b95d9d4aa198707a649268ef91c")?;

    let result = provider.handle_request(ProviderRequest::with_single(
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
