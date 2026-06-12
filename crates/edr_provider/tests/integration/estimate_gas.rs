#![cfg(feature = "test-utils")]

use std::{num::NonZeroU64, str::FromStr, sync::Arc};

use edr_chain_l1::{
    rpc::{call::L1CallRequest, receipt::L1RpcTransactionReceipt, TransactionRequest},
    L1ChainSpec,
};
use edr_chain_spec::EvmSpecId;
use edr_primitives::{bytes, Address, Bytes, B256, U256, U64};
use edr_provider::{
    config::GasEstimationMode,
    test_utils::{create_test_config, deploy_contract},
    time::CurrentTime,
    MethodInvocation, NoopLogger, Provider, ProviderError, ProviderRequest,
    TransactionFailureReason,
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

const HIGH_GAS_REQUIRED_BYTECODE: &str =
    include_str!("../../../../data/deployment_bytecode/HighGasRequiredContract.bin");

struct Fixture {
    deployed_address: Address,
    from: Address,
    provider: Provider<L1ChainSpec>,
}

fn new_fixture(gas_estimation_mode: GasEstimationMode, bytecode: &str) -> Fixture {
    let mut config = create_test_config();
    config.gas_estimation_mode = gas_estimation_mode;

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

// When transaction_gas_cap is set, REVM rejects any gas value above it with
// TxGasLimitGreaterThanCap. The binary search previously used the block gas
// limit (30M) as its upper bound; when the search minimum sits near the cap,
// probes exceed it and estimation errors. The cap here is the EIP-7825 default
// for Osaka+ (2^24 ≈ 16.7M). HighGasRequiredContract forces the minimum close
// to it via `require(gasleft() >= 16_700_000)`.
#[tokio::test(flavor = "multi_thread")]
async fn binary_search_does_not_probe_above_transaction_gas_cap() -> anyhow::Result<()> {
    let mut config = create_test_config();
    config.hardfork = EvmSpecId::OSAKA;
    // Mirrors the default behaviour of the napi layer: transaction_gas_cap and
    // default_transaction_gas_limit are both derived from the hardfork.
    let transaction_gas_cap = edr_eip7825::transaction_gas_cap_for_hardfork(EvmSpecId::OSAKA)
        .expect("Osaka activates EIP-7825");
    config.transaction_gas_cap = Some(transaction_gas_cap);
    config.default_transaction_gas_limit =
        NonZeroU64::new(transaction_gas_cap).expect("cap is non-zero");

    let caller = public_key_to_address(
        config
            .owned_accounts
            .first_mut()
            .expect("account")
            .public_key(),
    );
    let provider = Provider::<L1ChainSpec>::new(
        runtime::Handle::current(),
        Box::new(NoopLogger::<L1ChainSpec>::default()),
        Box::new(|_| {}),
        config,
        Arc::new(RwLock::<ContractDecoder>::default()),
        CurrentTime,
    )?;
    let contract = deploy_contract(
        &provider,
        caller,
        Bytes::from_str(HIGH_GAS_REQUIRED_BYTECODE)?,
    )?;

    let estimate_response = provider
        .handle_request(ProviderRequest::with_single(MethodInvocation::EstimateGas(
            L1CallRequest {
                from: Some(caller),
                to: Some(contract),
                data: Some(FUNCTION_TO_ESTIMATE_CALLDATA),
                ..L1CallRequest::default()
            },
            None,
        )))
        .expect("eth_estimateGas should succeed");

    let estimate = serde_json::from_value::<U64>(estimate_response.result)?.to::<u64>();
    assert!(
        estimate <= transaction_gas_cap,
        "estimateGas returned {estimate}, which exceeds the transaction gas cap {transaction_gas_cap}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn oog_free_estimation_is_used_when_mode_is_avoid_internal_out_of_gas() -> anyhow::Result<()>
{
    let fixture = new_fixture(
        GasEstimationMode::AvoidInternalOutOfGas,
        INTERNAL_OOG_BYTECODE,
    );

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
async fn naive_estimation_internally_oogs() -> anyhow::Result<()> {
    let fixture_naive = new_fixture(GasEstimationMode::Naive, INTERNAL_OOG_BYTECODE);
    let naive_estimation = estimate_gas_for_function_to_estimate(&fixture_naive);
    invoke_function_to_estimate_with_gas(&fixture_naive, naive_estimation);
    let n_naive = read_n(&fixture_naive);

    // With the naive mode, the estimation is just small enough that the inner
    // sub-call OOGs, so `n` stays zero.
    assert_eq!(
        n_naive,
        U256::ZERO,
        "expected naive estimation to internally OOG (n stays zero)",
    );

    // The avoid-internal-out-of-gas mode should produce a strictly larger
    // estimation.
    let fixture_oog_free = new_fixture(
        GasEstimationMode::AvoidInternalOutOfGas,
        INTERNAL_OOG_BYTECODE,
    );
    let oog_free_estimation = estimate_gas_for_function_to_estimate(&fixture_oog_free);
    assert!(
        oog_free_estimation > naive_estimation,
        "OOG-free estimation {oog_free_estimation} should exceed naive {naive_estimation}",
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn error_when_no_oog_free_estimation_exists() -> anyhow::Result<()> {
    let fixture_oog_free = new_fixture(
        GasEstimationMode::AvoidInternalOutOfGas,
        ALWAYS_INTERNAL_OOG_BYTECODE,
    );

    // With AvoidInternalOutOfGas we have nowhere to go: the inner call OOGs at
    // any gas limit, so the estimation must error instead of returning a value
    // that internally OOGs.
    let error = fixture_oog_free
        .provider
        .handle_request(ProviderRequest::with_single(MethodInvocation::EstimateGas(
            L1CallRequest {
                from: Some(fixture_oog_free.from),
                to: Some(fixture_oog_free.deployed_address),
                data: Some(FUNCTION_TO_ESTIMATE_CALLDATA),
                ..L1CallRequest::default()
            },
            None,
        )))
        .expect_err("estimation should error when no OOG-free value exists");

    assert!(
        matches!(
            &error,
            ProviderError::TransactionFailed(failure)
                if matches!(
                    failure.failure.reason,
                    TransactionFailureReason::InternalCallOutOfGas
                )
        ),
        "expected an internal call out of gas failure, got: {error:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn plain_transfer_estimation_unchanged_with_avoid_internal_out_of_gas() -> anyhow::Result<()>
{
    let mut config = create_test_config();
    config.gas_estimation_mode = GasEstimationMode::AvoidInternalOutOfGas;
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
    // is that changing the estimation mode does not affect this output.
    assert_eq!(gas.into_limbs()[0], 21_001);

    Ok(())
}
