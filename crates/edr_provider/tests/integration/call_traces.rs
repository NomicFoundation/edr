#![cfg(feature = "test-utils")]

use std::{str::FromStr, sync::Arc};

use edr_chain_l1::{rpc::TransactionRequest, L1ChainSpec};
use edr_primitives::{Bytes, B256};
use edr_provider::{
    test_utils::{create_test_config, deploy_contract},
    time::CurrentTime,
    MethodInvocation, NoopLogger, Provider, ProviderRequest,
};
use edr_signer::public_key_to_address;
use edr_solidity::{config::IncludeTraces, contract_decoder::ContractDecoder};
use foundry_evm_traces::TraceMemberOrder;
use parking_lot::RwLock;
use tokio::runtime;

// Deployment bytecode of a contract whose runtime code emits
// `LOG1(topic = 0x00..deadbeef)` with empty data and stops, regardless of
// calldata:
//
// 6027600c60003960276000f3  copy the 0x27-byte runtime to memory and return it
// 7f00..deadbeef            PUSH32 topic
// 6000 6000 a1 00           LOG1(offset = 0, size = 0, topic); STOP
const LOG_EMITTING_CONTRACT_BYTECODE: &str = "0x6027600c60003960276000f37f00000000000000000000000000000000000000000000000000000000deadbeef60006000a100";

const LOG_TOPIC: B256 = B256::new({
    let mut topic = [0u8; 32];
    topic[28] = 0xde;
    topic[29] = 0xad;
    topic[30] = 0xbe;
    topic[31] = 0xef;
    topic
});

// https://github.com/NomicFoundation/edr/issues/1542
async fn transaction_call_trace_includes_logs(verbose_raw_tracing: bool) -> anyhow::Result<()> {
    let mut config = create_test_config();
    config.observability.include_call_traces = IncludeTraces::All;
    config.observability.verbose_raw_tracing = verbose_raw_tracing;

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

    let deployed_address = deploy_contract(
        &provider,
        from,
        Bytes::from_str(LOG_EMITTING_CONTRACT_BYTECODE)?,
    )?;

    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SendTransaction(TransactionRequest {
            from,
            to: Some(deployed_address),
            ..TransactionRequest::default()
        }),
    ))?;

    let arena = response
        .call_trace_arenas
        .first()
        .expect("Transaction should have a call trace");

    let root_node = arena.nodes().first().expect("Arena should have a root");
    assert_eq!(
        root_node.logs.len(),
        1,
        "expected the emitted event in the root call's logs, got: {:?}",
        root_node.logs
    );
    let log = root_node.logs.first().expect("logs are non-empty");
    assert_eq!(log.raw_log.topics(), [LOG_TOPIC]);
    assert!(
        root_node.ordering.contains(&TraceMemberOrder::Log(0)),
        "expected a log entry in the trace ordering, got: {:?}",
        root_node.ordering
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn transaction_call_trace_includes_logs_default_tracing() -> anyhow::Result<()> {
    transaction_call_trace_includes_logs(false).await
}

#[tokio::test(flavor = "multi_thread")]
async fn transaction_call_trace_includes_logs_verbose_raw_tracing() -> anyhow::Result<()> {
    transaction_call_trace_includes_logs(true).await
}
