#![cfg(feature = "test-utils")]

use std::sync::{Arc, LazyLock};

use edr_chain_l1::{
    rpc::{call::L1CallRequest, TransactionRequest},
    L1ChainSpec,
};
use edr_primitives::{bytes, Bytes, HashSet};
use edr_provider::{
    test_utils::{create_test_config, deploy_contract},
    time::CurrentTime,
    MethodInvocation, NoopLogger, Provider, ProviderRequest,
};
use edr_signer::public_key_to_address;
use edr_solidity::contract_decoder::ContractDecoder;
use parking_lot::{Mutex, RwLock};
use tokio::runtime;

use crate::common::compile::{instrument_and_compile, InstrumentAndCompileResult};

static COMPILED: LazyLock<InstrumentAndCompileResult> = LazyLock::new(|| {
    let source = include_str!("../../../../data/contracts/test/CoverageTest.sol");
    instrument_and_compile(source, "CoverageTest.sol")
});

#[derive(Default)]
struct CoverageReporter {
    hits: HashSet<Bytes>,
}

struct Fixture {
    from: edr_primitives::Address,
    provider: Provider<L1ChainSpec>,
}

fn create_provider_with_bail(
    coverage_reporter: Arc<Mutex<CoverageReporter>>,
    bail_on_failure: bool,
) -> Fixture {
    let mut config = create_test_config();
    config.bail_on_transaction_failure = bail_on_failure;
    config.observability.on_collected_coverage_fn = Some(Box::new(move |hits| {
        coverage_reporter.lock().hits.extend(hits);
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

    Fixture { from, provider }
}

#[tokio::test(flavor = "multi_thread")]
async fn contract_call_returns_expected_output() -> anyhow::Result<()> {
    let coverage_reporter = Arc::new(Mutex::default());
    let Fixture { from, provider } = create_provider_with_bail(coverage_reporter.clone(), false);

    let bytecode = COMPILED.contracts["CoverageCall"].bytecode.clone();
    let deployed_address = deploy_contract(&provider, from, bytecode)?;

    // cast calldata 'function getValue()' => 0x20965255
    let calldata: Bytes = bytes!("0x20965255");

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

    // The return value should be abi-encoded uint256(42) = 0x2a padded to 32 bytes
    let result: String = serde_json::from_value(response.result)?;
    let expected_return = "0x000000000000000000000000000000000000000000000000000000000000002a";
    assert_eq!(result, expected_return, "getValue() should return 42");

    let reporter = coverage_reporter.lock();
    assert!(
        !reporter.hits.is_empty(),
        "coverage hits should be reported for a successful call"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn contract_call_reverts_with_expected_reason() -> anyhow::Result<()> {
    let coverage_reporter = Arc::new(Mutex::default());
    let Fixture { from, provider } = create_provider_with_bail(coverage_reporter.clone(), true);

    // Deploy with bail off (deployment succeeds), then call with bail on
    // We need a separate provider for deployment since bail_on_transaction_failure
    // applies to all transactions. Instead, deploy CoverageCall first (it won't
    // revert on deploy), then call willRevert().
    let bytecode = COMPILED.contracts["CoverageCall"].bytecode.clone();
    let deployed_address = deploy_contract(&provider, from, bytecode)?;

    // cast calldata 'function willRevert()' => 0x73ee93b3
    let calldata: Bytes = bytes!("0x73ee93b3");

    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SendTransaction(TransactionRequest {
            from,
            to: Some(deployed_address),
            data: Some(calldata),
            ..TransactionRequest::default()
        }),
    ));

    // With bail_on_transaction_failure=true, the provider returns an error
    let err = response.expect_err("willRevert() should fail");
    let err_string = format!("{err}");
    assert!(
        err_string.contains("expected revert reason"),
        "error should contain the revert reason, got: {err_string}"
    );

    let reporter = coverage_reporter.lock();
    assert!(
        !reporter.hits.is_empty(),
        "coverage hits should be reported even for a reverting call"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn contract_successfully_deploys() -> anyhow::Result<()> {
    let coverage_reporter = Arc::new(Mutex::default());
    let Fixture { from, provider } = create_provider_with_bail(coverage_reporter.clone(), false);

    let bytecode = COMPILED.contracts["CoverageDeploySuccess"].bytecode.clone();
    let deployed_address = deploy_contract(&provider, from, bytecode)?;

    assert_ne!(
        deployed_address,
        edr_primitives::Address::ZERO,
        "contract should deploy to a non-zero address"
    );

    let reporter = coverage_reporter.lock();
    assert!(
        !reporter.hits.is_empty(),
        "coverage hits should be reported for constructor execution"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn contract_reverts_on_deployment() -> anyhow::Result<()> {
    let coverage_reporter = Arc::new(Mutex::default());
    let Fixture { from, provider } = create_provider_with_bail(coverage_reporter.clone(), true);

    let bytecode = COMPILED.contracts["CoverageDeployRevert"].bytecode.clone();

    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SendTransaction(TransactionRequest {
            from,
            data: Some(bytecode),
            ..TransactionRequest::default()
        }),
    ));

    // With bail_on_transaction_failure=true, the provider returns an error
    let err = response.expect_err("deploying CoverageDeployRevert should fail");
    let err_string = format!("{err}");
    assert!(
        err_string.contains("constructor failed"),
        "error should contain the constructor revert reason, got: {err_string}"
    );

    let reporter = coverage_reporter.lock();
    assert!(
        !reporter.hits.is_empty(),
        "coverage hits should be reported even for a reverting constructor"
    );

    Ok(())
}
