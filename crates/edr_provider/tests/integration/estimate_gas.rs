#![cfg(feature = "test-utils")]

use std::{num::NonZeroU64, str::FromStr, sync::Arc};

use edr_chain_l1::{
    rpc::{call::L1CallRequest, TransactionRequest},
    L1ChainSpec,
};
use edr_chain_spec::EvmSpecId;
use edr_primitives::{bytes, Address, Bytes, U256, U64};
use edr_provider::{
    config::{GasEstimationMode, ProviderConfig},
    test_utils::{create_test_config, deploy_contract},
    time::CurrentTime,
    MethodInvocation, NoopLogger, Provider, ProviderError, ProviderErrorForChainSpec,
    ProviderRequest, TransactionFailureReason,
};
use edr_signer::public_key_to_address;
use edr_solidity::contract_decoder::ContractDecoder;
use parking_lot::RwLock;
use tokio::runtime;

const INTERNAL_OOG_BYTECODE: &str =
    include_str!("../../../../data/deployment_bytecode/InternalOOGContract.bin");
const ALWAYS_INTERNAL_OOG_BYTECODE: &str =
    include_str!("../../../../data/deployment_bytecode/AlwaysInternalOOGContract.bin");
const HIGH_GAS_REQUIRED_BYTECODE: &str =
    include_str!("../../../../data/deployment_bytecode/HighGasRequiredContract.bin");

// `cast sig 'functionToEstimate()'`
const FUNCTION_TO_ESTIMATE_CALLDATA: Bytes = bytes!("0x1b6cdb67");
// `cast sig 'n()'`
const N_CALLDATA: Bytes = bytes!("0x2e52d606");

fn new_provider(
    config: ProviderConfig<edr_chain_l1::Hardfork>,
) -> anyhow::Result<(Provider<L1ChainSpec>, Address)> {
    let from = public_key_to_address(
        config
            .owned_accounts
            .first()
            .expect("config should have an account")
            .public_key(),
    );
    let provider = Provider::new(
        runtime::Handle::current(),
        Box::new(NoopLogger::<L1ChainSpec>::default()),
        Box::new(|_event| {}),
        config,
        Arc::new(RwLock::<ContractDecoder>::default()),
        CurrentTime,
    )?;
    Ok((provider, from))
}

/// A provider with a contract deployed from the given bytecode. All the test
/// contracts expose a `functionToEstimate()` entry point.
struct Fixture {
    deployed_address: Address,
    from: Address,
    provider: Provider<L1ChainSpec>,
}

impl Fixture {
    fn new(config: ProviderConfig<edr_chain_l1::Hardfork>, bytecode: &str) -> anyhow::Result<Self> {
        let (provider, from) = new_provider(config)?;
        let deployed_address = deploy_contract(&provider, from, Bytes::from_str(bytecode)?)?;

        Ok(Self {
            deployed_address,
            from,
            provider,
        })
    }

    fn with_estimation_mode(mode: GasEstimationMode, bytecode: &str) -> anyhow::Result<Self> {
        let mut config = create_test_config();
        config.gas_estimation_mode = mode;
        Self::new(config, bytecode)
    }

    /// Estimates the gas of calling `functionToEstimate()` on the deployed
    /// contract.
    fn estimate_gas(&self) -> Result<u64, ProviderErrorForChainSpec<L1ChainSpec>> {
        let response = self.provider.handle_request(ProviderRequest::with_single(
            MethodInvocation::EstimateGas(
                L1CallRequest {
                    from: Some(self.from),
                    to: Some(self.deployed_address),
                    data: Some(FUNCTION_TO_ESTIMATE_CALLDATA),
                    ..L1CallRequest::default()
                },
                None,
            ),
        ))?;

        let gas: U64 = serde_json::from_value(response.result).expect("estimate should be U64");
        Ok(gas.to::<u64>())
    }

    /// Sends a transaction calling `functionToEstimate()` with the given gas
    /// limit.
    fn invoke_with_gas(&self, gas: u64) {
        self.provider
            .handle_request(ProviderRequest::with_single(
                MethodInvocation::SendTransaction(TransactionRequest {
                    from: self.from,
                    to: Some(self.deployed_address),
                    data: Some(FUNCTION_TO_ESTIMATE_CALLDATA),
                    gas: Some(gas),
                    ..TransactionRequest::default()
                }),
            ))
            .expect("eth_sendTransaction should succeed");
    }

    /// Reads the contract's `n` counter, which is only incremented when the
    /// inner sub-call runs to completion.
    fn read_n(&self) -> U256 {
        let response = self
            .provider
            .handle_request(ProviderRequest::with_single(MethodInvocation::Call(
                L1CallRequest {
                    from: Some(self.from),
                    to: Some(self.deployed_address),
                    data: Some(N_CALLDATA),
                    ..L1CallRequest::default()
                },
                None,
                None,
            )))
            .expect("eth_call n() should succeed");

        let bytes: Bytes =
            serde_json::from_value(response.result).expect("call should return Bytes");
        U256::from_be_slice(bytes.as_ref())
    }
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

    let fixture = Fixture::new(config, HIGH_GAS_REQUIRED_BYTECODE)?;

    let estimate = fixture.estimate_gas()?;
    assert!(
        estimate <= transaction_gas_cap,
        "estimateGas returned {estimate}, which exceeds the transaction gas cap {transaction_gas_cap}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn oog_free_estimation_is_used_when_mode_is_avoid_internal_out_of_gas() -> anyhow::Result<()>
{
    let fixture = Fixture::with_estimation_mode(
        GasEstimationMode::AvoidInternalOutOfGas,
        INTERNAL_OOG_BYTECODE,
    )?;

    let estimation = fixture.estimate_gas()?;
    fixture.invoke_with_gas(estimation);

    // `useGas` only increments `n` when it runs to completion. A non-zero `n`
    // proves the inner sub-call did not OOG.
    let n = fixture.read_n();
    assert!(
        n > U256::ZERO,
        "expected `n` to be non-zero after OOG-free estimation, got {n}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn naive_estimation_internally_oogs() -> anyhow::Result<()> {
    let fixture_naive =
        Fixture::with_estimation_mode(GasEstimationMode::Naive, INTERNAL_OOG_BYTECODE)?;
    let naive_estimation = fixture_naive.estimate_gas()?;
    fixture_naive.invoke_with_gas(naive_estimation);

    // With the naive mode, the estimation is just small enough that the inner
    // sub-call OOGs, so `n` stays zero.
    assert_eq!(
        fixture_naive.read_n(),
        U256::ZERO,
        "expected naive estimation to internally OOG (n stays zero)",
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn error_when_no_oog_free_estimation_exists() -> anyhow::Result<()> {
    let fixture = Fixture::with_estimation_mode(
        GasEstimationMode::AvoidInternalOutOfGas,
        ALWAYS_INTERNAL_OOG_BYTECODE,
    )?;

    // With AvoidInternalOutOfGas we have nowhere to go: the inner call OOGs at
    // any gas limit, so the estimation must error instead of returning a value
    // that internally OOGs.
    let error = fixture
        .estimate_gas()
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
    let (provider, from) = new_provider(config)?;

    let response =
        provider.handle_request(ProviderRequest::with_single(MethodInvocation::EstimateGas(
            L1CallRequest {
                from: Some(from),
                to: Some(Address::ZERO),
                value: Some(U256::from(1u64)),
                ..L1CallRequest::default()
            },
            None,
        )))?;

    let gas: U64 = serde_json::from_value(response.result)?;
    // The same value `estimate_gas` produces today for a plain transfer:
    // `minimum_cost + 1` (the clamp in `estimate_gas`). What matters here
    // is that changing the estimation mode does not affect this output.
    assert_eq!(gas.to::<u64>(), 21_001);

    Ok(())
}
