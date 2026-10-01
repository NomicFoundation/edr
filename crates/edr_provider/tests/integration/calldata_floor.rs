#![cfg(feature = "test-utils")]

mod eip7623;
mod eip7976;

use edr_chain_l1::{rpc::TransactionRequest, L1ChainSpec};
use edr_provider::Provider;

use crate::common::provider::{estimate_gas, gas_used, send_transaction};

/// Sends the transaction and asserts the `gasUsed` reported by its receipt.
fn assert_transaction_gas_usage(
    provider: &Provider<L1ChainSpec>,
    request: TransactionRequest,
    expected_gas_usage: u64,
) {
    let transaction_hash = send_transaction(provider, request).expect("transaction should succeed");

    let gas_used = gas_used(provider, transaction_hash);
    assert_eq!(gas_used, expected_gas_usage);
}
