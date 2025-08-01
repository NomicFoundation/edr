#![cfg(feature = "test-remote")]
use std::str::FromStr as _;

use edr_eth::{
    address,
    l1::{self, InvalidHeader, L1ChainSpec},
    transaction::TransactionValidation,
    B256,
};
use edr_evm::{
    hardfork::{self, ChainOverride},
    transaction::TransactionError,
};
use edr_generic::GenericChainSpec;
use edr_provider::{
    time::CurrentTime, DebugTraceError, MethodInvocation, Provider, ProviderError, ProviderRequest,
    ProviderSpec, SyncProviderSpec,
};
use edr_test_utils::env::get_alchemy_url;
use serial_test::serial;

use crate::integration::helpers::get_chain_fork_provider;

// Arbitrum block after Cancun activation
// that does not have fields required by Cancun
// `excessBlobGas` or `blobGasUsed`
const CANCUN_BLOCK_NUMBER: u64 = 361_518_399;

fn get_provider<
    ChainSpecT: SyncProviderSpec<
            CurrentTime,
            BlockEnv: Default,
            Hardfork = l1::SpecId,
            SignedTransaction: Default
                                   + TransactionValidation<
                ValidationError: From<l1::InvalidTransaction> + PartialEq,
            >,
        > + ProviderSpec<CurrentTime>,
>(
    hardfork: l1::SpecId,
    block_number: u64,
) -> anyhow::Result<Provider<ChainSpecT>> {
    // Arbitrum one
    const CHAIN_ID: u64 = 42161;

    let chain_override = ChainOverride {
        name: "Arbitrum".to_owned(),
        hardfork_activation_overrides: Some(hardfork::Activations::with_spec_id(hardfork)),
    };
    let url = get_alchemy_url().replace("eth-mainnet", "arb-mainnet");
    // THIS CALL IS UNSAFE AND MIGHT LEAD TO UNDEFINED BEHAVIOR. WE DEEM THE RISK
    // ACCEPTABLE FOR TESTING PURPOSES ONLY.
    unsafe { std::env::set_var("__EDR_UNSAFE_SKIP_UNSUPPORTED_TRANSACTION_TYPES", "true") };
    let provider =
        get_chain_fork_provider::<ChainSpecT>(CHAIN_ID, block_number, chain_override, url);
    // THIS CALL IS UNSAFE AND MIGHT LEAD TO UNDEFINED BEHAVIOR. WE DEEM THE RISK
    // ACCEPTABLE FOR TESTING PURPOSES ONLY.
    unsafe { std::env::remove_var("__EDR_UNSAFE_SKIP_UNSUPPORTED_TRANSACTION_TYPES") };
    provider
}

// `eth_debugTraceTransaction` should succeed
// even if block header does not contain `excess_blob_gas` in Cancun or above
// https://github.com/NomicFoundation/edr/issues/947
#[serial]
#[tokio::test(flavor = "multi_thread")]
async fn issue_947_generic_evm_should_default_excess_gas() -> anyhow::Result<()> {
    let provider = get_provider::<GenericChainSpec>(l1::SpecId::CANCUN, CANCUN_BLOCK_NUMBER)?;

    let transaction_hash =
        B256::from_str("0x9fccb755176d48b3e5e576aff003bb5dc4aeefa8b0b22e082555bdc705276278")?;

    let result = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::DebugTraceTransaction(transaction_hash, None),
    ));

    // The block does not have the excess blob gas information
    // but the execution should succeed since edr should define a
    // default for GenericChainSpec
    assert!(result.is_ok());

    Ok(())
}

// `eth_debugTraceTransaction` should fail on l1 chain if
// block header does not contain `excess_blob_gas` in Cancun or above
// https://github.com/NomicFoundation/edr/issues/947
#[serial]
#[tokio::test(flavor = "multi_thread")]
async fn issue_947_should_fail_with_missing_blob_gas_on_l1_after_cancun() -> anyhow::Result<()> {
    let provider = get_provider::<L1ChainSpec>(l1::SpecId::CANCUN, CANCUN_BLOCK_NUMBER)?;

    let transaction_hash =
        B256::from_str("0x9fccb755176d48b3e5e576aff003bb5dc4aeefa8b0b22e082555bdc705276278")?;

    let result = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::DebugTraceTransaction(transaction_hash, None),
    ));

    // The block does not have the excess blob gas information
    // so the execution should fail since L1ChainSpec should not allow it
    assert!(matches!(
        result,
        Err(ProviderError::DebugTrace(
            DebugTraceError::TransactionError(TransactionError::InvalidHeader(
                InvalidHeader::ExcessBlobGasNotSet
            ))
        ))
    ));

    Ok(())
}

// `eth_debugTraceTransaction` should succeed on generic chain if
// block header does not contain `excess_blob_gas` below Cancun
// https://github.com/NomicFoundation/edr/issues/947
#[serial]
#[tokio::test(flavor = "multi_thread")]
async fn issue_947_should_succeed_on_generic_before_cancun() -> anyhow::Result<()> {
    // Arbitrum block after shanghai activation
    let shanghai_arbitrum_block = 184_097_481;
    let provider = get_provider::<GenericChainSpec>(l1::SpecId::SHANGHAI, shanghai_arbitrum_block)?;

    let result = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SendTransaction(edr_rpc_eth::TransactionRequest {
            from: address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
            to: Some(address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")),
            ..edr_rpc_eth::TransactionRequest::default()
        }),
    ));

    assert!(result.is_ok());

    Ok(())
}
