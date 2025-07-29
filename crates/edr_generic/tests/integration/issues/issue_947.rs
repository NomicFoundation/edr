#![cfg(feature = "test-remote")]
use std::str::FromStr as _;

use edr_eth::{
    l1::{self, InvalidHeader, L1ChainSpec}, transaction::TransactionValidation, B256
};
use edr_evm::{hardfork::{self, ChainOverride}, 
    transaction::TransactionError}
;
use edr_generic::GenericChainSpec;
use edr_provider::{
    time::CurrentTime, DebugTraceError, MethodInvocation, Provider, ProviderError, ProviderRequest, ProviderSpec, SyncProviderSpec
};
use serial_test::serial;

use crate::integration::helpers::get_chain_fork_provider;


fn get_provider<ChainSpecT: SyncProviderSpec<
CurrentTime,
BlockEnv: Default,
Hardfork = l1::SpecId,
SignedTransaction: Default
+ TransactionValidation<
ValidationError: From<l1::InvalidTransaction> + PartialEq,
>,
> + ProviderSpec<CurrentTime>>()  -> anyhow::Result<Provider<ChainSpecT>> { 
    // Arbitrum one
    const CHAIN_ID: u64 = 42161;
    const BLOCK_NUMBER: u64 = 361_518_399;
    
    let chain_override = ChainOverride {
            name: "Arbitrum".to_owned(),
            hardfork_activation_overrides: Some(hardfork::Activations::with_spec_id(
                l1::SpecId::CANCUN,
            )),
        };
    // THIS CALL IS UNSAFE AND MIGHT LEAD TO UNDEFINED BEHAVIOR. WE DEEM THE RISK
    // ACCEPTABLE FOR TESTING PURPOSES ONLY.
    unsafe { std::env::set_var("__EDR_UNSAFE_SKIP_UNSUPPORTED_TRANSACTION_TYPES", "true") };
    let provider = get_chain_fork_provider::<ChainSpecT>(CHAIN_ID, BLOCK_NUMBER, chain_override, Some("arb-mainnet"));
     // THIS CALL IS UNSAFE AND MIGHT LEAD TO UNDEFINED BEHAVIOR. WE DEEM THE RISK
    // ACCEPTABLE FOR TESTING PURPOSES ONLY.
    unsafe { std::env::remove_var("__EDR_UNSAFE_SKIP_UNSUPPORTED_TRANSACTION_TYPES") };
    provider
}

// TODO: should we "replicate" the data from arbitrum in case
// they decide to follow the specification in the future?

// TODO: test that it's setting the right default BlobExcessGas value

// `eth_debugTraceTransaction` should succeed
// even if block header does not contain `excess_blob_gas` in Cancun or above
// https://github.com/NomicFoundation/edr/issues/947
#[serial]
#[tokio::test(flavor = "multi_thread")]
async fn issue_947_generic_evm_should_default_excess_gas() -> anyhow::Result<()> {
    let provider = get_provider::<GenericChainSpec>()?;

    let transaction_hash =
        B256::from_str("0x9fccb755176d48b3e5e576aff003bb5dc4aeefa8b0b22e082555bdc705276278")?;

    let result = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::DebugTraceTransaction(transaction_hash, None),
    ));

    assert!(result.is_ok());

    Ok(())
}

// `eth_debugTraceTransaction` should fail on l1 chain if
// block header does not contain `excess_blob_gas` in Cancun or above
// https://github.com/NomicFoundation/edr/issues/947
#[serial]
#[tokio::test(flavor = "multi_thread")]
async fn issue_947_should_fail_on_l1() -> anyhow::Result<()> {
    let provider = get_provider::<L1ChainSpec>()?;

    let transaction_hash =
        B256::from_str("0x9fccb755176d48b3e5e576aff003bb5dc4aeefa8b0b22e082555bdc705276278")?;

    let result = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::DebugTraceTransaction(transaction_hash, None),
    ));

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

// TODO: should we add test for op-stack chain as well?
// doubt: edr_generic crate does not depend on edr_op
