#![cfg(feature = "test-utils")]

use std::{num::NonZeroU64, str::FromStr, sync::Arc};

use edr_chain_l1::{rpc::call::L1CallRequest, L1ChainSpec};
use edr_chain_spec::EvmSpecId;
use edr_primitives::{bytes, Bytes, U64};
use edr_provider::{
    test_utils::{create_test_config, deploy_contract},
    time::CurrentTime,
    MethodInvocation, NoopLogger, Provider, ProviderRequest,
};
use edr_signer::public_key_to_address;
use edr_solidity::contract_decoder::ContractDecoder;
use parking_lot::RwLock;
use tokio::runtime;

const HIGH_GAS_REQUIRED_BYTECODE: &str =
    include_str!("../../../../data/deployment_bytecode/HighGasRequiredContract.bin");

// `cast sig 'functionToEstimate()'`
const FUNCTION_TO_ESTIMATE_CALLDATA: Bytes = bytes!("0x1b6cdb67");

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
