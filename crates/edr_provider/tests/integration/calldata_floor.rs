#![cfg(feature = "test-utils")]

mod eip7623;
mod eip7976;

use edr_chain_l1::{
    rpc::{call::L1CallRequest, receipt::L1RpcTransactionReceipt, TransactionRequest},
    L1ChainSpec,
};
use edr_primitives::{B256, U64};
use edr_provider::{MethodInvocation, Provider, ProviderRequest};

use crate::common::provider::send_transaction;

fn assert_transaction_gas_usage(
    provider: &Provider<L1ChainSpec>,
    request: TransactionRequest,
    expected_gas_usage: u64,
) {
    let transaction_hash = send_transaction(provider, request).expect("transaction should succeed");

    let gas_used = gas_used(provider, transaction_hash);
    assert_eq!(gas_used, expected_gas_usage);
}

/// Estimates the request's gas usage via `eth_estimateGas`.
fn estimate_gas(provider: &Provider<L1ChainSpec>, request: L1CallRequest) -> u64 {
    let response = provider
        .handle_request(ProviderRequest::with_single(MethodInvocation::EstimateGas(
            request, None,
        )))
        .expect("eth_estimateGas should succeed");

    let gas: U64 = serde_json::from_value(response.result).expect("response should be U64");

    gas.into_limbs()[0]
}

/// Returns the `gasUsed` from the transaction's receipt.
fn gas_used(provider: &Provider<L1ChainSpec>, transaction_hash: B256) -> u64 {
    let response = provider
        .handle_request(ProviderRequest::with_single(
            MethodInvocation::GetTransactionReceipt(transaction_hash),
        ))
        .expect("eth_getTransactionReceipt should succeed");

    let receipt: Option<L1RpcTransactionReceipt> =
        serde_json::from_value(response.result).expect("response should be Receipt");

    let receipt = receipt.expect("receipt should exist");

    receipt.gas_used
}
