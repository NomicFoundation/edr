mod deploy_contract;
mod send_data_to_eoa;

use core::convert::Infallible;
use std::sync::Arc;

use edr_eth::{receipt::BlockReceipt, transaction::EthTransactionRequest, SpecId, B256, U64};
use edr_provider::{
    test_utils::{create_test_config, one_ether},
    time::CurrentTime,
    AccountConfig, MethodInvocation, NoopLogger, Provider, ProviderRequest,
};
use edr_rpc_eth::CallRequest;
use edr_solidity::contract_decoder::ContractDecoder;
use edr_test_utils::secret_key::secret_key_from_str;
use tokio::runtime;

const CHAIN_ID: u64 = 0x7a69;

fn assert_transaction_gas_usage(
    provider: &Provider<Infallible>,
    request: EthTransactionRequest,
    expected_gas_usage: u64,
) {
    let transaction_hash = send_transaction(provider, request).expect("transaction should succeed");

    let gas_used = gas_used(provider, transaction_hash);
    assert_eq!(gas_used, expected_gas_usage);
}

fn estimate_gas(provider: &Provider<Infallible>, request: CallRequest) -> u64 {
    let response = provider
        .handle_request(ProviderRequest::Single(MethodInvocation::EstimateGas(
            request, None,
        )))
        .expect("eth_estimateGas should succeed");

    let gas: U64 = serde_json::from_value(response.result).expect("response should be U64");

    gas.into_limbs()[0]
}

fn gas_used(provider: &Provider<Infallible>, transaction_hash: B256) -> u64 {
    let response = provider
        .handle_request(ProviderRequest::Single(
            MethodInvocation::GetTransactionReceipt(transaction_hash),
        ))
        .expect("eth_getTransactionReceipt should succeed");

    let receipt: Option<BlockReceipt> =
        serde_json::from_value(response.result).expect("response should be Receipt");

    let receipt = receipt.expect("receipt should exist");

    receipt.gas_used
}

fn new_provider(hardfork: SpecId) -> anyhow::Result<Provider<Infallible>> {
    let secret_key = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;

    let logger = Box::new(NoopLogger);
    let subscriber = Box::new(|_event| {});

    let mut config = create_test_config();
    config.accounts = vec![AccountConfig {
        secret_key,
        balance: one_ether(),
    }];
    config.chain_id = CHAIN_ID;
    config.hardfork = hardfork;

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?;

    Ok(provider)
}

fn send_transaction(
    provider: &Provider<Infallible>,
    request: EthTransactionRequest,
) -> anyhow::Result<B256> {
    let response = provider
        .handle_request(ProviderRequest::Single(MethodInvocation::SendTransaction(
            request,
        )))
        .expect("eth_sendTransaction should succeed");

    let transaction_hash: B256 = serde_json::from_value(response.result)?;

    Ok(transaction_hash)
}
