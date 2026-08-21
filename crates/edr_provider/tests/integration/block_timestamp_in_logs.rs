#![cfg(feature = "test-utils")]

use std::{str::FromStr as _, sync::Arc};

use edr_chain_l1::{rpc::TransactionRequest, L1ChainSpec};
use edr_eth::{filter::LogFilterOptions, BlockSpec, PreEip1898BlockSpec};
use edr_primitives::{Address, Bytes, B256};
use edr_provider::{
    test_utils::{create_test_config_with, MinimalProviderConfig},
    time::CurrentTime,
    MethodInvocation, NoopLogger, Provider, ProviderRequest,
};
use edr_solidity::contract_decoder::ContractDecoder;
use parking_lot::RwLock;
use tokio::runtime;

/// `blockTimestamp` on logs, per <https://github.com/ethereum/execution-apis/pull/639>.
///
/// The field exists so that a consumer reading logs by range does not need a
/// second `eth_getBlockByHash` per block just to timestamp them. That saving is
/// most acute in a browser, where a provider often cannot batch those calls at
/// all and each one is its own round-trip.
///
/// The value has to be the timestamp of the block the log is actually in, not
/// the latest block, so this asserts it against `eth_getBlockByNumber` rather
/// than merely asserting the field is present.
///
/// Deployment code for a contract whose fallback emits one anonymous log:
/// `MSTORE(0, 1); LOG0(0, 32)`.
const LOG_EMITTER_DEPLOYMENT_BYTECODE: &str = "0x600a600c600039600a6000f3600160005260206000a000";

#[tokio::test(flavor = "multi_thread")]
async fn logs_carry_the_timestamp_of_their_own_block() -> anyhow::Result<()> {
    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});

    let config = create_test_config_with(MinimalProviderConfig::local_with_accounts());

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::new(RwLock::<ContractDecoder>::default()),
        CurrentTime,
    )?;

    let from = {
        let response = provider
            .handle_request(ProviderRequest::with_single(MethodInvocation::Accounts(())))?;
        let accounts: Vec<Address> = serde_json::from_value(response.result)?;
        accounts[0]
    };

    let deploy_hash = {
        let response = provider.handle_request(ProviderRequest::with_single(
            MethodInvocation::SendTransaction(TransactionRequest {
                from,
                data: Some(Bytes::from_str(LOG_EMITTER_DEPLOYMENT_BYTECODE)?),
                ..TransactionRequest::default()
            }),
        ))?;
        serde_json::from_value::<B256>(response.result)?
    };

    let deployed_address = {
        let response = provider.handle_request(ProviderRequest::with_single(
            MethodInvocation::GetTransactionReceipt(deploy_hash),
        ))?;
        let receipt: serde_json::Value = serde_json::from_value(response.result)?;
        Address::from_str(
            receipt["contractAddress"]
                .as_str()
                .expect("contract address"),
        )?
    };

    // Two calls, so the logs land in two DIFFERENT blocks: a single-block test
    // would pass even if every log were stamped with the latest block's time.
    for _ in 0..2 {
        provider.handle_request(ProviderRequest::with_single(
            MethodInvocation::SendTransaction(TransactionRequest {
                from,
                to: Some(deployed_address),
                ..TransactionRequest::default()
            }),
        ))?;
    }

    let logs = {
        let response = provider.handle_request(ProviderRequest::with_single(
            MethodInvocation::GetLogs(LogFilterOptions {
                from_block: Some(BlockSpec::Number(0)),
                to_block: Some(BlockSpec::latest()),
                ..LogFilterOptions::default()
            }),
        ))?;
        serde_json::from_value::<Vec<serde_json::Value>>(response.result)?
    };

    assert_eq!(logs.len(), 2, "expected one log per call");

    let mut seen = Vec::new();
    for log in &logs {
        let block_timestamp = log
            .get("blockTimestamp")
            .unwrap_or_else(|| panic!("log has no blockTimestamp field: {log}"));
        let block_timestamp = block_timestamp
            .as_str()
            .expect("blockTimestamp must be a hex QUANTITY string, as the spec requires");
        assert!(
            block_timestamp.starts_with("0x"),
            "blockTimestamp must be a 0x-prefixed QUANTITY, got {block_timestamp}"
        );

        // ...and it must be THIS block's timestamp
        let block_number = log["blockNumber"].as_str().expect("blockNumber");
        let response = provider.handle_request(ProviderRequest::with_single(
            MethodInvocation::GetBlockByNumber(
                PreEip1898BlockSpec::Number(u64::from_str_radix(
                    block_number.trim_start_matches("0x"),
                    16,
                )?),
                false,
            ),
        ))?;
        let block: serde_json::Value = serde_json::from_value(response.result)?;
        assert_eq!(
            block["timestamp"].as_str().expect("block timestamp"),
            block_timestamp,
            "log's blockTimestamp does not match the timestamp of block {block_number}"
        );

        seen.push((block_number.to_owned(), block_timestamp.to_owned()));
    }

    assert_ne!(
        seen[0].0, seen[1].0,
        "the two logs should be in different blocks, or this proves nothing"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn receipt_logs_carry_the_block_timestamp_too() -> anyhow::Result<()> {
    // The spec has ONE `Log` schema, shared by `eth_getLogs` and
    // `ReceiptInfo.logs`, so the field belongs on both rather than only on the
    // filter path.
    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});

    let config = create_test_config_with(MinimalProviderConfig::local_with_accounts());

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::new(RwLock::<ContractDecoder>::default()),
        CurrentTime,
    )?;

    let from = {
        let response = provider
            .handle_request(ProviderRequest::with_single(MethodInvocation::Accounts(())))?;
        let accounts: Vec<Address> = serde_json::from_value(response.result)?;
        accounts[0]
    };

    let deploy_hash = {
        let response = provider.handle_request(ProviderRequest::with_single(
            MethodInvocation::SendTransaction(TransactionRequest {
                from,
                data: Some(Bytes::from_str(LOG_EMITTER_DEPLOYMENT_BYTECODE)?),
                ..TransactionRequest::default()
            }),
        ))?;
        serde_json::from_value::<B256>(response.result)?
    };

    let deployed_address = {
        let response = provider.handle_request(ProviderRequest::with_single(
            MethodInvocation::GetTransactionReceipt(deploy_hash),
        ))?;
        let receipt: serde_json::Value = serde_json::from_value(response.result)?;
        Address::from_str(
            receipt["contractAddress"]
                .as_str()
                .expect("contract address"),
        )?
    };

    let call_hash = {
        let response = provider.handle_request(ProviderRequest::with_single(
            MethodInvocation::SendTransaction(TransactionRequest {
                from,
                to: Some(deployed_address),
                ..TransactionRequest::default()
            }),
        ))?;
        serde_json::from_value::<B256>(response.result)?
    };

    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetTransactionReceipt(call_hash),
    ))?;
    let receipt: serde_json::Value = serde_json::from_value(response.result)?;
    let log = &receipt["logs"][0];

    let block_timestamp = log
        .get("blockTimestamp")
        .unwrap_or_else(|| panic!("receipt log has no blockTimestamp field: {log}"))
        .as_str()
        .expect("blockTimestamp must be a hex QUANTITY string");

    // compared against the block itself, not against another field of the same
    // response, so the assertion cannot pass by comparing a value with itself
    let block_number = receipt["blockNumber"].as_str().expect("blockNumber");
    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetBlockByNumber(
            PreEip1898BlockSpec::Number(u64::from_str_radix(
                block_number.trim_start_matches("0x"),
                16,
            )?),
            false,
        ),
    ))?;
    let block: serde_json::Value = serde_json::from_value(response.result)?;
    assert_eq!(
        block["timestamp"].as_str().expect("block timestamp"),
        block_timestamp
    );

    Ok(())
}
