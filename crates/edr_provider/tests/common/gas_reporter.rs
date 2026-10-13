//! A contract that reports the gas limit of the call that executes it.
#![cfg(feature = "test-utils")]

use edr_chain_l1::{
    rpc::{call::L1CallRequest, TransactionRequest},
    L1ChainSpec,
};
use edr_eth::BlockSpec;
use edr_primitives::{address, bytes, Address, Bytes, U256};
use edr_provider::{MethodInvocation, Provider, ProviderRequest};

use crate::common::provider::{quantity_field, send_transaction};

pub const CALLER: Address = address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
pub const GAS_REPORTER_ADDRESS: Address = address!("0x1000000000000000000000000000000000000001");
/// Returns the gas left after executing `GAS` as a 32-byte word.
const GAS_REPORTER_CODE: Bytes = bytes!("5a60005260206000f3");
const TX_BASE_COST: u64 = 21_000;
const GAS_OPCODE_COST: u64 = 2;
/// The gas left that `GAS` reports excludes its own cost.
const GAS_SPENT_BEFORE_REPORT: u64 = TX_BASE_COST + GAS_OPCODE_COST;

/// Deploys the gas reporter at `GAS_REPORTER_ADDRESS`.
pub fn deploy_gas_reporter(provider: &Provider<L1ChainSpec>) -> anyhow::Result<()> {
    provider.handle_request(ProviderRequest::with_single(MethodInvocation::SetCode(
        GAS_REPORTER_ADDRESS,
        GAS_REPORTER_CODE,
    )))?;

    Ok(())
}

/// Calls the gas reporter without a `gas` value and returns its gas limit.
pub fn call_without_gas(
    provider: &Provider<L1ChainSpec>,
    block_spec: Option<BlockSpec>,
) -> anyhow::Result<u64> {
    let request = L1CallRequest {
        from: Some(CALLER),
        to: Some(GAS_REPORTER_ADDRESS),
        ..L1CallRequest::default()
    };

    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::Call(request, block_spec, None),
    ))?;

    let output: Bytes = response.deserialize_result()?;
    let gas_left: u64 = U256::try_from_be_slice(&output)
        .ok_or_else(|| anyhow::anyhow!("output should fit in a 32-byte word"))?
        .to();

    Ok(gas_left + GAS_SPENT_BEFORE_REPORT)
}

/// Sends a transaction to the gas reporter without a `gas` value and returns
/// its gas limit.
pub fn send_transaction_without_gas(provider: &Provider<L1ChainSpec>) -> anyhow::Result<u64> {
    let transaction_hash = send_transaction(
        provider,
        TransactionRequest {
            from: CALLER,
            to: Some(GAS_REPORTER_ADDRESS),
            ..TransactionRequest::default()
        },
    )?;

    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetTransactionByHash(transaction_hash),
    ))?;

    let transaction: Option<serde_json::Value> = response.deserialize_result()?;
    let transaction = transaction.ok_or_else(|| anyhow::anyhow!("transaction should exist"))?;

    quantity_field(&transaction, "gas")
}
