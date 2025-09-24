#![cfg(feature = "test-utils")]

use std::{str::FromStr as _, sync::Arc};

use edr_chain_l1::{
    rpc::{call::L1CallRequest, receipt::L1BlockReceipt, TransactionRequest},
    L1ChainSpec,
};
use edr_primitives::{bytes, Address, Bytes, HashSet, B256};
use edr_provider::{
    gas_reports::GasReport, test_utils::create_test_config, time::CurrentTime, MethodInvocation,
    NoopLogger, Provider, ProviderRequest,
};
use edr_signer::public_key_to_address;
use edr_solidity::contract_decoder::ContractDecoder;
use parking_lot::Mutex;
use tokio::runtime;

const INCREMENT_DEPLOYED_BYTECODE: &str =
    include_str!("../../../../data/deployed_bytecode/increment.in");

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
        Arc::<ContractDecoder>::default(),
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
                        Bytes::from_str(INCREMENT_DEPLOYED_BYTECODE).expect("Invalid bytecode"),
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

        let receipt: L1BlockReceipt = serde_json::from_value(response.result)
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
