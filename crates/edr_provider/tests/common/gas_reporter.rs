//! A contract that reports the gas limit of the call that executes it.
#![cfg(feature = "test-utils")]

use edr_chain_l1::{
    request::Legacy,
    rpc::{call::L1CallRequest, TransactionRequest},
    L1ChainSpec, L1SignedTransaction,
};
use edr_chain_spec::EvmSpecId;
use edr_eth::BlockSpec;
use edr_primitives::{address, Address, Bytes, U256};
use edr_provider::{MethodInvocation, Provider, ProviderRequest};
use edr_rpc_eth::{AccountOverrideOptions, StateOverrideOptions};
use edr_runtime::transaction::calculate_initial_tx_gas_for_tx;
use edr_transaction::TxKind;

use crate::common::{
    bytecode::{opcode::GAS, BytecodeBuilder},
    provider::{quantity_field, send_transaction},
};

pub const CALLER: Address = address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
pub const GAS_REPORTER_ADDRESS: Address = address!("0x1000000000000000000000000000000000000001");
/// Returns the gas left after executing `GAS` as a 32-byte word.
fn gas_reporter_code() -> Bytes {
    let mut code = BytecodeBuilder::default();
    code.opcode(GAS).return_top_word();
    code.runtime()
}
/// The gas left that `GAS` reports excludes its own cost.
const GAS_OPCODE_COST: u64 = 2;

/// Deploys the gas reporter at `GAS_REPORTER_ADDRESS`.
pub fn deploy_gas_reporter(provider: &Provider<L1ChainSpec>) -> anyhow::Result<()> {
    provider.handle_request(ProviderRequest::with_single(MethodInvocation::SetCode(
        GAS_REPORTER_ADDRESS,
        gas_reporter_code(),
    )))?;

    Ok(())
}

/// Returns the intrinsic gas of a call from `CALLER` to the gas reporter in
/// `hardfork`.
fn reporter_call_intrinsic_gas(hardfork: edr_chain_l1::Hardfork) -> u64 {
    let spec_id = EvmSpecId::from(hardfork);
    // Intrinsic gas ignores the nonce, gas price and gas limit.
    let transaction: L1SignedTransaction = Legacy {
        nonce: 0,
        gas_price: 0,
        gas_limit: 0,
        kind: TxKind::Call(GAS_REPORTER_ADDRESS),
        value: U256::ZERO,
        input: Bytes::new(),
    }
    .fake_sign(CALLER)
    .into();

    calculate_initial_tx_gas_for_tx(
        &transaction,
        spec_id,
        edr_eip2780::transaction_intrinsic_gas_info_for_hardfork(&transaction, spec_id),
    )
    .initial_total_gas()
}

/// Calls the gas reporter without a `gas` value and returns the gas limit
/// the call ran with.
///
/// The gas limit is reconstructed from `gasleft()` plus the intrinsic gas in
/// `hardfork`, which must be the hardfork of the executing block. The call
/// overrides the reporter's code, so it works at blocks before the reporter
/// was deployed.
pub fn call_without_gas(
    provider: &Provider<L1ChainSpec>,
    block_spec: Option<BlockSpec>,
    hardfork: edr_chain_l1::Hardfork,
) -> anyhow::Result<u64> {
    let request = L1CallRequest {
        from: Some(CALLER),
        to: Some(GAS_REPORTER_ADDRESS),
        ..L1CallRequest::default()
    };
    let state_overrides = StateOverrideOptions::from_iter([(
        GAS_REPORTER_ADDRESS,
        AccountOverrideOptions {
            balance: None,
            nonce: None,
            code: Some(gas_reporter_code()),
            storage: None,
            storage_diff: None,
        },
    )]);

    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::Call(request, block_spec, Some(state_overrides)),
    ))?;

    let output: Bytes = response.deserialize_result()?;
    let gas_left: u64 = U256::try_from_be_slice(&output)
        .ok_or_else(|| anyhow::anyhow!("output should fit in a 32-byte word"))?
        .to();

    Ok(gas_left + GAS_OPCODE_COST + reporter_call_intrinsic_gas(hardfork))
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
