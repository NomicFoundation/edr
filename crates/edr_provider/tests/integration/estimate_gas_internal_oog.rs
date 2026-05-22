#![cfg(feature = "test-utils")]

use std::{str::FromStr, sync::Arc};

use edr_chain_l1::{
    rpc::{call::L1CallRequest, receipt::L1RpcTransactionReceipt, TransactionRequest},
    L1ChainSpec,
};
use edr_primitives::{bytes, Address, Bytes, B256, U256, U64};
use edr_provider::{
    test_utils::create_test_config, time::CurrentTime, MethodInvocation, NoopLogger, Provider,
    ProviderRequest,
};
use edr_signer::public_key_to_address;
use edr_solidity::contract_decoder::ContractDecoder;
use parking_lot::RwLock;
use tokio::runtime;

const INTERNAL_OOG_BYTECODE: &str =
    include_str!("../../../../data/deployment_bytecode/InternalOOGContract.bin");
const ALWAYS_INTERNAL_OOG_BYTECODE: &str =
    include_str!("../../../../data/deployment_bytecode/AlwaysInternalOOGContract.bin");

// `cast sig 'functionToEstimate()'`
const FUNCTION_TO_ESTIMATE_CALLDATA: Bytes = bytes!("0x1b6cdb67");
// `cast sig 'n()'`
const N_CALLDATA: Bytes = bytes!("0x2e52d606");

struct Fixture {
    deployed_address: Address,
    from: Address,
    provider: Provider<L1ChainSpec>,
}

fn new_fixture(avoid_internal_oog: bool, bytecode: &str) -> Fixture {
    let mut config = create_test_config();
    config.estimate_gas_avoid_internal_oog = avoid_internal_oog;

    let from = {
        let secret_key = config
            .owned_accounts
            .first_mut()
            .expect("should have an account");
        public_key_to_address(secret_key.public_key())
    };

    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});
    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::new(RwLock::<ContractDecoder>::default()),
        CurrentTime,
    )
    .expect("provider construction should succeed");

    let deploy_response = provider
        .handle_request(ProviderRequest::with_single(
            MethodInvocation::SendTransaction(TransactionRequest {
                from,
                data: Some(Bytes::from_str(bytecode).expect("hex-encoded bytecode")),
                ..TransactionRequest::default()
            }),
        ))
        .expect("contract deployment should succeed");

    let tx_hash: B256 =
        serde_json::from_value(deploy_response.result).expect("deployment hash should decode");

    let receipt_response = provider
        .handle_request(ProviderRequest::with_single(
            MethodInvocation::GetTransactionReceipt(tx_hash),
        ))
        .expect("receipt fetch should succeed");

    let receipt: L1RpcTransactionReceipt =
        serde_json::from_value(receipt_response.result).expect("receipt should decode");

    let deployed_address = receipt
        .contract_address
        .expect("deployment should produce a contract address");

    Fixture {
        deployed_address,
        from,
        provider,
    }
}

fn estimate_gas_for_function_to_estimate(fixture: &Fixture) -> u64 {
    let response = fixture
        .provider
        .handle_request(ProviderRequest::with_single(MethodInvocation::EstimateGas(
            L1CallRequest {
                from: Some(fixture.from),
                to: Some(fixture.deployed_address),
                data: Some(FUNCTION_TO_ESTIMATE_CALLDATA),
                ..L1CallRequest::default()
            },
            None,
        )))
        .expect("eth_estimateGas should succeed");
    let gas: U64 = serde_json::from_value(response.result).expect("estimate should be U64");
    gas.into_limbs()[0]
}

fn invoke_function_to_estimate_with_gas(fixture: &Fixture, gas: u64) {
    fixture
        .provider
        .handle_request(ProviderRequest::with_single(
            MethodInvocation::SendTransaction(TransactionRequest {
                from: fixture.from,
                to: Some(fixture.deployed_address),
                data: Some(FUNCTION_TO_ESTIMATE_CALLDATA),
                gas: Some(gas),
                ..TransactionRequest::default()
            }),
        ))
        .expect("eth_sendTransaction should succeed");
}

fn read_n(fixture: &Fixture) -> U256 {
    let response = fixture
        .provider
        .handle_request(ProviderRequest::with_single(MethodInvocation::Call(
            L1CallRequest {
                from: Some(fixture.from),
                to: Some(fixture.deployed_address),
                data: Some(N_CALLDATA),
                ..L1CallRequest::default()
            },
            None,
            None,
        )))
        .expect("eth_call n() should succeed");
    let bytes: Bytes = serde_json::from_value(response.result).expect("call should return Bytes");
    U256::from_be_slice(bytes.as_ref())
}

#[tokio::test(flavor = "multi_thread")]
async fn oog_free_estimation_is_used_when_flag_is_on() -> anyhow::Result<()> {
    let fixture = new_fixture(true, INTERNAL_OOG_BYTECODE);

    let estimation = estimate_gas_for_function_to_estimate(&fixture);
    invoke_function_to_estimate_with_gas(&fixture, estimation);

    // `useGas` only increments `n` when it runs to completion. A non-zero `n`
    // proves the inner sub-call did not OOG.
    let n = read_n(&fixture);
    assert!(
        n > U256::ZERO,
        "expected `n` to be non-zero after OOG-free estimation, got {n}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn legacy_estimation_internally_oogs_when_flag_is_off() -> anyhow::Result<()> {
    let fixture_off = new_fixture(false, INTERNAL_OOG_BYTECODE);
    let legacy_estimation = estimate_gas_for_function_to_estimate(&fixture_off);
    invoke_function_to_estimate_with_gas(&fixture_off, legacy_estimation);
    let n_off = read_n(&fixture_off);

    // Reproduce today's behavior: with the flag off, the estimation is just
    // small enough that the inner sub-call OOGs, so `n` stays zero.
    assert_eq!(
        n_off,
        U256::ZERO,
        "expected legacy estimation to internally OOG (n stays zero)",
    );

    // And the new behavior should produce a strictly larger estimation.
    let fixture_on = new_fixture(true, INTERNAL_OOG_BYTECODE);
    let oog_free_estimation = estimate_gas_for_function_to_estimate(&fixture_on);
    assert!(
        oog_free_estimation > legacy_estimation,
        "OOG-free estimation {oog_free_estimation} should exceed legacy {legacy_estimation}",
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn fallback_when_no_oog_free_estimation_exists() -> anyhow::Result<()> {
    let fixture_on = new_fixture(true, ALWAYS_INTERNAL_OOG_BYTECODE);
    let estimation_on = estimate_gas_for_function_to_estimate(&fixture_on);

    let fixture_off = new_fixture(false, ALWAYS_INTERNAL_OOG_BYTECODE);
    let estimation_off = estimate_gas_for_function_to_estimate(&fixture_off);

    // Even with the flag on we have nowhere to go: the inner call OOGs at any
    // gas limit. The estimation must fall back to the legacy value.
    assert_eq!(
        estimation_on, estimation_off,
        "estimation with flag on should fall back to legacy estimation",
    );

    // The estimation should still be sendable (outer call catches the inner
    // failure) and `n` will remain zero, confirming the fallback path.
    invoke_function_to_estimate_with_gas(&fixture_on, estimation_on);
    assert_eq!(read_n(&fixture_on), U256::ZERO);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn plain_transfer_estimation_unchanged_with_flag_on() -> anyhow::Result<()> {
    let mut config = create_test_config();
    config.estimate_gas_avoid_internal_oog = true;
    let from = {
        let secret_key = config
            .owned_accounts
            .first_mut()
            .expect("should have an account");
        public_key_to_address(secret_key.public_key())
    };

    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});
    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::new(RwLock::<ContractDecoder>::default()),
        CurrentTime,
    )
    .expect("provider construction should succeed");

    let response = provider
        .handle_request(ProviderRequest::with_single(MethodInvocation::EstimateGas(
            L1CallRequest {
                from: Some(from),
                to: Some(Address::ZERO),
                value: Some(U256::from(1u64)),
                ..L1CallRequest::default()
            },
            None,
        )))
        .expect("plain transfer estimation should succeed");
    let gas: U64 = serde_json::from_value(response.result).expect("U64");
    // The same value `estimate_gas` produces today for a plain transfer:
    // `minimum_cost + 1` (the `<=` clamp in `estimate_gas`). What matters here
    // is that turning the flag on does not change this output.
    assert_eq!(gas.into_limbs()[0], 21_001);

    Ok(())
}
