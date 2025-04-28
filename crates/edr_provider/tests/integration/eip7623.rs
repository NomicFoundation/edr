#![cfg(feature = "test-utils")]

mod deploy_contract;
mod send_data_to_eoa;

use std::sync::Arc;

use edr_eth::{
    B256, U64,
    l1::{self, L1ChainSpec},
};
use edr_provider::{
    MethodInvocation, NoopLogger, Provider, ProviderRequest,
    config::OwnedAccount,
    test_utils::{create_test_config, one_ether},
    time::CurrentTime,
};
use edr_rpc_eth::{CallRequest, TransactionRequest};
use edr_solidity::contract_decoder::ContractDecoder;
use edr_test_utils::secret_key::secret_key_from_str;
use tokio::runtime;

const CHAIN_ID: u64 = 0x7a69;

fn assert_transaction_gas_usage(
    provider: &Provider<L1ChainSpec>,
    request: TransactionRequest,
    expected_gas_usage: u64,
) {
    let transaction_hash = send_transaction(provider, request).expect("transaction should succeed");

    let gas_used = gas_used(provider, transaction_hash);
    assert_eq!(gas_used, expected_gas_usage);
}

fn estimate_gas(provider: &Provider<L1ChainSpec>, request: CallRequest) -> u64 {
    let response = provider
        .handle_request(ProviderRequest::Single(MethodInvocation::EstimateGas(
            request, None,
        )))
        .expect("eth_estimateGas should succeed");

    let gas: U64 = serde_json::from_value(response.result).expect("response should be U64");

    gas.into_limbs()[0]
}

fn gas_used(provider: &Provider<L1ChainSpec>, transaction_hash: B256) -> u64 {
    let response = provider
        .handle_request(ProviderRequest::Single(
            MethodInvocation::GetTransactionReceipt(transaction_hash),
        ))
        .expect("eth_getTransactionReceipt should succeed");

    let receipt: Option<edr_rpc_eth::receipt::Block> =
        serde_json::from_value(response.result).expect("response should be Receipt");

    let receipt = receipt.expect("receipt should exist");

    receipt.gas_used
}

fn new_provider(hardfork: l1::SpecId) -> anyhow::Result<Provider<L1ChainSpec>> {
    let secret_key = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;

    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});

    let mut config = create_test_config();
    config.accounts = vec![OwnedAccount {
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
    provider: &Provider<L1ChainSpec>,
    request: TransactionRequest,
) -> anyhow::Result<B256> {
    let response = provider
        .handle_request(ProviderRequest::Single(MethodInvocation::SendTransaction(
            request,
        )))
        .expect("eth_sendTransaction should succeed");

    let transaction_hash: B256 = serde_json::from_value(response.result)?;

    Ok(transaction_hash)
}
