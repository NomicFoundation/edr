#![cfg(feature = "test-utils")]

use std::{str::FromStr as _, sync::Arc};

use edr_chain_l1::{
    rpc::{call::L1CallRequest, receipt::L1RpcTransactionReceipt, TransactionRequest},
    L1ChainSpec,
};
use edr_gas_report::GasReport;
use edr_primitives::{bytes, Address, Bytes, HashSet, B256};
use edr_provider::{
    test_utils::create_test_config, time::CurrentTime, MethodInvocation, NoopLogger, Provider,
    ProviderRequest,
};
use edr_signer::public_key_to_address;
use edr_solidity::contract_decoder::ContractDecoder;
use parking_lot::{Mutex, RwLock};
use tokio::runtime;

const INCREMENT_CREATION_BYTECODE: &str =
    include_str!("../../../../data/creation_bytecode/Increment.bin");

// > cast calldata 'function incBy(uint)' 1
const INCREMENT_CALLDATA: Bytes =
    bytes!("0x70119d060000000000000000000000000000000000000000000000000000000000000001");

#[derive(Default)]
struct CoverageReporter {
    hits: HashSet<Bytes>,
}

#[derive(Default)]
pub struct GasReporter {
    report: GasReport,
}

struct Fixture {
    deployed_address: Address,
    from: Address,
    provider: Provider<L1ChainSpec>,
}

fn assert_hits(reporter: &CoverageReporter) {
    assert_eq!(reporter.hits.len(), 2);
    assert_eq!(
        reporter.hits,
        [
            bytes!("0x0000000000000000000000000000000000000000000000000000000000000001"),
            bytes!("0x0000000000000000000000000000000000000000000000000000000000000002")
        ]
        .into_iter()
        .collect()
    );
}

fn provider_with_deployed_test_contract(
    coverage_reporter: Arc<Mutex<CoverageReporter>>,
    gas_reporter: Arc<Mutex<GasReporter>>,
) -> Fixture {
    let mut config = create_test_config();
    config.observability.on_collected_coverage_fn = Some(Box::new(move |hits| {
        coverage_reporter.lock().hits.extend(hits);

        Ok(())
    }));
    config.observability.on_collected_gas_report_fn = Some(Box::new(move |report| {
        let mut gas_reporter = gas_reporter.lock();
        gas_reporter.report.merge(report);
        Ok(())
    }));

    let from = {
        let secret_key = config
            .owned_accounts
            .first_mut()
            .expect("should have an account");

        public_key_to_address(secret_key.public_key())
    };

    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});
    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::new(RwLock::<ContractDecoder>::default()),
        CurrentTime,
    )
    .expect("Failed to construct provider");

    // Deploy test contract
    let transaction_hash = {
        let response = provider
            .handle_request(ProviderRequest::with_single(
                MethodInvocation::SendTransaction(TransactionRequest {
                    from,
                    data: Some(
                        Bytes::from_str(INCREMENT_CREATION_BYTECODE).expect("Invalid bytecode"),
                    ),
                    ..TransactionRequest::default()
                }),
            ))
            .expect("Failed to deploy test contract");

        serde_json::from_value::<B256>(response.result)
            .expect("Failed to deserialize transaction hash")
    };

    // Retrieve the deployed address
    let deployed_address = {
        let response = provider
            .handle_request(ProviderRequest::with_single(
                MethodInvocation::GetTransactionReceipt(transaction_hash),
            ))
            .expect("Failed to get transaction receipt");

        let receipt: L1RpcTransactionReceipt = serde_json::from_value(response.result)
            .expect("Failed to deserialize transaction receipt");

        receipt
            .contract_address
            .expect("Failed to get contract address")
    };

    Fixture {
        deployed_address,
        from,
        provider,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn call() -> anyhow::Result<()> {
    let coverage_reporter = Arc::new(Mutex::default());
    let gas_reporter = Arc::new(Mutex::default());

    let Fixture {
        deployed_address,
        from,
        provider,
    } = provider_with_deployed_test_contract(coverage_reporter.clone(), gas_reporter.clone());

    let _response =
        provider.handle_request(ProviderRequest::with_single(MethodInvocation::Call(
            L1CallRequest {
                from: Some(from),
                to: Some(deployed_address),
                data: Some(INCREMENT_CALLDATA),
                ..L1CallRequest::default()
            },
            None,
            None,
        )))?;

    assert_hits(&coverage_reporter.lock());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn debug_trace_call() -> anyhow::Result<()> {
    let coverage_reporter = Arc::new(Mutex::default());
    let gas_reporter = Arc::new(Mutex::default());

    let Fixture {
        deployed_address,
        from,
        provider,
    } = provider_with_deployed_test_contract(coverage_reporter.clone(), gas_reporter.clone());

    let _response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::DebugTraceCall(
            L1CallRequest {
                from: Some(from),
                to: Some(deployed_address),
                data: Some(INCREMENT_CALLDATA),
                ..L1CallRequest::default()
            },
            None,
            None,
        ),
    ))?;

    assert_hits(&coverage_reporter.lock());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn debug_trace_transaction() -> anyhow::Result<()> {
    let coverage_reporter = Arc::new(Mutex::default());
    let gas_reporter = Arc::new(Mutex::default());

    let Fixture {
        deployed_address,
        from,
        provider,
    } = provider_with_deployed_test_contract(coverage_reporter.clone(), gas_reporter.clone());

    let transaction_hash: B256 = {
        let response = provider.handle_request(ProviderRequest::with_single(
            MethodInvocation::SendTransaction(TransactionRequest {
                from,
                to: Some(deployed_address),
                data: Some(INCREMENT_CALLDATA),
                ..TransactionRequest::default()
            }),
        ))?;

        serde_json::from_value(response.result).expect("Failed to deserialize transaction hash")
    };

    // Reset the hits after the transaction
    coverage_reporter.lock().hits.clear();

    let _response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::DebugTraceTransaction(transaction_hash, None),
    ))?;

    assert_hits(&coverage_reporter.lock());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn estimate_gas() -> anyhow::Result<()> {
    let coverage_reporter = Arc::new(Mutex::default());
    let gas_reporter = Arc::new(Mutex::default());

    let Fixture {
        deployed_address,
        from,
        provider,
    } = provider_with_deployed_test_contract(coverage_reporter.clone(), gas_reporter.clone());

    let _response =
        provider.handle_request(ProviderRequest::with_single(MethodInvocation::EstimateGas(
            L1CallRequest {
                from: Some(from),
                to: Some(deployed_address),
                data: Some(INCREMENT_CALLDATA),
                ..L1CallRequest::default()
            },
            None,
        )))?;

    assert_hits(&coverage_reporter.lock());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn send_transaction() -> anyhow::Result<()> {
    let coverage_reporter = Arc::new(Mutex::default());
    let gas_reporter = Arc::new(Mutex::default());

    let Fixture {
        deployed_address,
        from,
        provider,
    } = provider_with_deployed_test_contract(coverage_reporter.clone(), gas_reporter.clone());

    let _response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SendTransaction(TransactionRequest {
            from,
            to: Some(deployed_address),
            data: Some(INCREMENT_CALLDATA),
            ..TransactionRequest::default()
        }),
    ))?;

    assert_hits(&coverage_reporter.lock());

    Ok(())
}

/// Tests that coverage instrumentation preserves the returndata buffer across
/// various scenarios (forwarded calls, reverts, child deployments).
mod returndata {
    use std::sync::Arc;

    use edr_chain_l1::{rpc::call::L1CallRequest, L1ChainSpec};
    use edr_primitives::{bytes, Bytes};
    use edr_provider::{
        test_utils::{create_test_config, deploy_contract},
        time::CurrentTime,
        MethodInvocation, NoopLogger, Provider, ProviderRequest,
    };
    use edr_signer::public_key_to_address;
    use parking_lot::RwLock;
    use tokio::runtime;

    const COVERAGE_CALL_BYTECODE: &str =
        include_str!("../../../../data/creation_bytecode/CoverageCall.bin");

    struct Fixture {
        from: edr_primitives::Address,
        provider: Provider<L1ChainSpec>,
    }

    fn create_provider_with_coverage() -> Fixture {
        let mut config = create_test_config();
        // We need to activate coverage measurement for these tests, but don't use the
        // result.
        config.observability.on_collected_coverage_fn = Some(Box::new(move |_hits| Ok(())));

        let from = {
            let secret_key = config
                .owned_accounts
                .first_mut()
                .expect("should have an account");
            public_key_to_address(secret_key.public_key())
        };

        let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
        let subscriber = Box::new(|_event| {});
        let provider = Provider::new(
            runtime::Handle::current(),
            logger,
            subscriber,
            config,
            Arc::new(RwLock::default()),
            CurrentTime,
        )
        .expect("Failed to construct provider");

        Fixture { from, provider }
    }

    fn coverage_call_bytecode() -> Bytes {
        let hex = COVERAGE_CALL_BYTECODE.trim().strip_prefix("0x").unwrap();
        Bytes::from(hex::decode(hex).expect("invalid hex in CoverageCall.bin"))
    }

    /// Decodes the revert reason from a hex-encoded `Error(string)` return
    /// value.
    fn decode_revert_reason(hex_result: &str) -> String {
        let bytes: Bytes = hex_result.parse().expect("invalid hex");
        let return_data = edr_solidity::return_data::ReturnData::new(&bytes);
        assert!(
            return_data.is_error_return_data(),
            "expected error revert data, got: {hex_result}"
        );
        return_data.decode_error().expect("failed to decode error")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn forward_successful_call() -> anyhow::Result<()> {
        let Fixture { from, provider } = create_provider_with_coverage();

        let bytecode = coverage_call_bytecode();
        let deployed_address = deploy_contract(&provider, from, bytecode)?;

        // forwardSuccessfulCall() => 0xc07303ab
        let calldata: Bytes = bytes!("0xc07303ab");

        let response =
            provider.handle_request(ProviderRequest::with_single(MethodInvocation::Call(
                L1CallRequest {
                    from: Some(from),
                    to: Some(deployed_address),
                    data: Some(calldata),
                    ..L1CallRequest::default()
                },
                None,
                None,
            )))?;

        // Target.getValue() returns uint256(42), forwarded via returndatacopy.
        let result: String = serde_json::from_value(response.result)?;
        let expected = format!("0x{:0>64}", hex::encode(42u32.to_be_bytes()));
        assert_eq!(result, expected, "forwardSuccessfulCall() should return 42");

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn forward_reverted_call() -> anyhow::Result<()> {
        let Fixture { from, provider } = create_provider_with_coverage();

        let bytecode = coverage_call_bytecode();
        let deployed_address = deploy_contract(&provider, from, bytecode)?;

        // forwardRevertedCall() => 0x4cc06e6d
        let calldata: Bytes = bytes!("0x4cc06e6d");

        let response =
            provider.handle_request(ProviderRequest::with_single(MethodInvocation::Call(
                L1CallRequest {
                    from: Some(from),
                    to: Some(deployed_address),
                    data: Some(calldata),
                    ..L1CallRequest::default()
                },
                None,
                None,
            )))?;

        // forwardRevertedCall() returns the raw ABI-encoded revert data from
        // Target.willRevert() via returndatacopy.
        let result: String = serde_json::from_value(response.result)?;
        let reason = decode_revert_reason(&result);
        assert_eq!(reason, "expected revert reason");

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn deploy_child() -> anyhow::Result<()> {
        let Fixture { from, provider } = create_provider_with_coverage();

        let bytecode = coverage_call_bytecode();
        let deployed_address = deploy_contract(&provider, from, bytecode)?;

        // deployChild() => 0x2053bfe6
        let calldata: Bytes = bytes!("0x2053bfe6");

        let response =
            provider.handle_request(ProviderRequest::with_single(MethodInvocation::Call(
                L1CallRequest {
                    from: Some(from),
                    to: Some(deployed_address),
                    data: Some(calldata),
                    ..L1CallRequest::default()
                },
                None,
                None,
            )))?;

        // The EVM does not populate returndata for successful deployments, so
        // returndatasize() is 0 and the raw assembly return produces empty output.
        let result: String = serde_json::from_value(response.result)?;
        assert_eq!(
            result, "0x",
            "deployChild() should return empty data after successful CREATE"
        );

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn deploy_reverting_child() -> anyhow::Result<()> {
        let Fixture { from, provider } = create_provider_with_coverage();

        let bytecode = coverage_call_bytecode();
        let deployed_address = deploy_contract(&provider, from, bytecode)?;

        // deployRevertingChild() => 0xe2a529b6
        let calldata: Bytes = bytes!("0xe2a529b6");

        let response =
            provider.handle_request(ProviderRequest::with_single(MethodInvocation::Call(
                L1CallRequest {
                    from: Some(from),
                    to: Some(deployed_address),
                    data: Some(calldata),
                    ..L1CallRequest::default()
                },
                None,
                None,
            )))?;

        // deployRevertingChild() returns the raw ABI-encoded revert data from the
        // failed CoverageDeployRevert constructor via returndatacopy.
        let result: String = serde_json::from_value(response.result)?;
        let reason = decode_revert_reason(&result);
        assert_eq!(reason, "constructor failed");

        Ok(())
    }
}
