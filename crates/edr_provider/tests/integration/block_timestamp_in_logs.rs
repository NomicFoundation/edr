#![cfg(feature = "test-utils")]

//! `blockTimestamp` on logs.
//! see <https://github.com/ethereum/execution-apis/pull/639>
//!
//! The value has to be the timestamp of the block the log is actually in, not
//! the latest block, so these tests assert it against `eth_getBlockByNumber`
//! rather than merely asserting the field is present.

use std::str::FromStr as _;

use edr_chain_l1::{
    rpc::{block::L1RpcBlock, TransactionRequest},
    L1ChainSpec,
};
use edr_eth::{filter::LogFilterOptions, BlockSpec, PreEip1898BlockSpec};
use edr_primitives::{Address, Bytes, B256};
use edr_provider::{
    test_utils::{create_test_config, deploy_contract},
    time::CurrentTime,
    MethodInvocation, Provider, ProviderRequest,
};
use edr_signer::public_key_to_address;

use crate::common::provider::new_provider_from_config;

/// Deployment code for a contract whose fallback emits one anonymous log:
/// `MSTORE(0, 1); LOG0(0, 32)`.
const LOG_EMITTER_DEPLOYMENT_BYTECODE: &str = "0x600b600c600039600b6000f3600160005260206000a000";

fn new_provider() -> anyhow::Result<(Provider<L1ChainSpec, CurrentTime>, Address)> {
    let config = create_test_config();

    let caller = public_key_to_address(
        config
            .owned_accounts
            .first()
            .expect("config should have an account")
            .public_key(),
    );

    let provider = new_provider_from_config(config)?;

    Ok((provider, caller))
}

/// Reads a hex QUANTITY field off a raw JSON object.
///
/// The logs are inspected as raw JSON rather than deserialized into
/// [`edr_eth::filter::LogOutput`] on purpose: a typed round-trip would accept a
/// decimal number or an absent field just as happily, and the point here is the
/// wire form the spec asks for.
fn quantity(value: &serde_json::Value, field: &str) -> anyhow::Result<u64> {
    let raw = value
        .get(field)
        .unwrap_or_else(|| panic!("no {field} field in {value}"))
        .as_str()
        .unwrap_or_else(|| panic!("{field} must be a hex QUANTITY string, got {value}"));

    let digits = raw
        .strip_prefix("0x")
        .unwrap_or_else(|| panic!("{field} must be a 0x-prefixed QUANTITY, got {raw}"));

    Ok(u64::from_str_radix(digits, 16)?)
}

fn block_timestamp(
    provider: &Provider<L1ChainSpec, CurrentTime>,
    block_number: u64,
) -> anyhow::Result<u64> {
    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::Number(block_number), false),
    ))?;

    let block: L1RpcBlock<B256> = response.deserialize_result()?;
    Ok(block.timestamp)
}

fn call_log_emitter(
    provider: &Provider<L1ChainSpec, CurrentTime>,
    caller: Address,
    contract: Address,
) -> anyhow::Result<B256> {
    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SendTransaction(TransactionRequest {
            from: caller,
            to: Some(contract),
            ..TransactionRequest::default()
        }),
    ))?;

    Ok(response.deserialize_result::<B256>()?)
}

#[tokio::test(flavor = "multi_thread")]
async fn logs_carry_the_timestamp_of_their_own_block() -> anyhow::Result<()> {
    let (provider, caller) = new_provider()?;

    let contract = deploy_contract(
        &provider,
        caller,
        Bytes::from_str(LOG_EMITTER_DEPLOYMENT_BYTECODE)?,
    )?;

    // Two calls, so the logs land in two different blocks: a single-block test
    // would pass even if every log were stamped with the latest block's time.
    for _ in 0..2 {
        call_log_emitter(&provider, caller, contract)?;
    }

    let logs = {
        let response = provider.handle_request(ProviderRequest::with_single(
            MethodInvocation::GetLogs(LogFilterOptions {
                from_block: Some(BlockSpec::Number(0)),
                to_block: Some(BlockSpec::latest()),
                ..LogFilterOptions::default()
            }),
        ))?;
        response.deserialize_result::<Vec<serde_json::Value>>()?
    };

    assert_eq!(logs.len(), 2, "expected one log per call");

    let mut seen = Vec::new();
    for log in &logs {
        let log_timestamp = quantity(log, "blockTimestamp")?;
        let block_number = quantity(log, "blockNumber")?;

        assert_eq!(
            block_timestamp(&provider, block_number)?,
            log_timestamp,
            "log's blockTimestamp does not match the timestamp of block {block_number}"
        );

        seen.push((block_number, log_timestamp));
    }

    assert_ne!(
        seen[0].0, seen[1].0,
        "the two logs should be in different blocks, or this proves nothing"
    );
    assert_ne!(
        seen[0].1, seen[1].1,
        "the two blocks should have different timestamps, or a log stamped with \
         the latest block's time would still pass"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn receipt_logs_carry_the_block_timestamp_too() -> anyhow::Result<()> {
    // The spec has one `Log` schema, shared by `eth_getLogs` and
    // `ReceiptInfo.logs`, so the field belongs on both rather than only on the
    // filter path.
    let (provider, caller) = new_provider()?;

    let contract = deploy_contract(
        &provider,
        caller,
        Bytes::from_str(LOG_EMITTER_DEPLOYMENT_BYTECODE)?,
    )?;

    let call_hash = call_log_emitter(&provider, caller, contract)?;

    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetTransactionReceipt(call_hash),
    ))?;
    let receipt: serde_json::Value = response.deserialize_result()?;

    let log_timestamp = quantity(&receipt["logs"][0], "blockTimestamp")?;

    // Compared against the block itself, not against another field of the same
    // response, so the assertion cannot pass by comparing a value with itself.
    let block_number = quantity(&receipt, "blockNumber")?;
    assert_eq!(block_timestamp(&provider, block_number)?, log_timestamp);

    Ok(())
}
