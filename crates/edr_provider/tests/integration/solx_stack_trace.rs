#![cfg(feature = "test-utils")]

//! Verifies the solx (DWARF) stack-trace path through the JSON-RPC
//! provider. Solidity-test runs are exercised by the JS parity sweep in
//! `js/integration-tests/solx-parity-sweep`; this file pins the
//! provider-side surface and exercises a small slice of the
//! [`StackTraceEntry`] variants we can hit from the existing solx fixtures.

use std::sync::Arc;

use anyhow::Context;
use edr_chain_l1::{
    rpc::{receipt::L1RpcTransactionReceipt, TransactionRequest},
    L1ChainSpec,
};
use edr_primitives::{hex, keccak256, Address, Bytes, Selector, B256};
use edr_provider::{
    test_utils::{create_test_config_with, MinimalProviderConfig},
    time::CurrentTime,
    MethodInvocation, NoopLogger, Provider, ProviderError, ProviderRequest,
};
use edr_signer::public_key_to_address;
use edr_solidity::{
    artifacts::{
        BuildInfoConfig, BuildInfoWithOutput, CompilerInput, CompilerOutput, SolxBytecode,
    },
    contract_decoder::ContractDecoder,
    debug_info::CompilerArtifact,
    solidity_stack_trace::{SourceReference, StackTraceCreationResult, StackTraceEntry},
};
use parking_lot::RwLock;
use tokio::runtime;

// ---------- build-info loaders ----------

fn solx_counter_build_info() -> anyhow::Result<(BuildInfoConfig, CompilerOutput<SolxBytecode>)> {
    let mut input: CompilerInput = serde_json::from_str(include_str!(
        "../../../edr_solidity/fixtures/solx_compiler_input.json"
    ))?;
    input.sources.get_mut("Counter.sol").unwrap().content =
        include_str!("../../../edr_solidity/fixtures/sources/Counter.sol").to_string();
    let output: CompilerOutput<SolxBytecode> = serde_json::from_str(include_str!(
        "../../../edr_solidity/fixtures/solx_compiler_output.json"
    ))?;
    let bi = BuildInfoWithOutput {
        _format: "hh3-sol-build-info-1".to_string(),
        id: "solx-counter".to_string(),
        solc_version: "0.8.34".to_string(),
        solc_long_version: "0.8.34+solx".to_string(),
        input: input.clone(),
        output: output.clone(),
    }
    .map_artifact(|b| -> Box<dyn CompilerArtifact> { Box::new(b) });
    Ok((
        BuildInfoConfig {
            build_infos: vec![bi],
            ignore_contracts: None,
        },
        output,
    ))
}

fn solx_scenarios_build_info() -> anyhow::Result<(BuildInfoConfig, CompilerOutput<SolxBytecode>)> {
    let mut input: CompilerInput = serde_json::from_str(include_str!(
        "../../../edr_solidity/fixtures/solx_compiler_input_scenarios.json"
    ))?;
    input
        .sources
        .get_mut("project/contracts/Scenarios.t.sol")
        .unwrap()
        .content =
        include_str!("../../../edr_solidity/fixtures/sources/Scenarios.t.sol").to_string();
    let output: CompilerOutput<SolxBytecode> = serde_json::from_str(include_str!(
        "../../../edr_solidity/fixtures/solx_compiler_output_scenarios.json"
    ))?;
    let bi = BuildInfoWithOutput {
        _format: "hh3-sol-build-info-1".to_string(),
        id: "solx-scenarios".to_string(),
        solc_version: "0.8.34".to_string(),
        solc_long_version: "0.8.34+solx".to_string(),
        input: input.clone(),
        output: output.clone(),
    }
    .map_artifact(|b| -> Box<dyn CompilerArtifact> { Box::new(b) });
    Ok((
        BuildInfoConfig {
            build_infos: vec![bi],
            ignore_contracts: None,
        },
        output,
    ))
}

// ---------- provider plumbing ----------

/// Builds a local provider seeded with `decoder`, with bail-on-failure set
/// so a reverting tx surfaces as [`ProviderError::TransactionFailed`].
fn make_provider(decoder: ContractDecoder) -> anyhow::Result<(Provider<L1ChainSpec>, Address)> {
    let mut config = create_test_config_with(MinimalProviderConfig::local_with_accounts());
    config.bail_on_transaction_failure = true;
    config.bail_on_call_failure = true;

    let from = public_key_to_address(
        config
            .owned_accounts
            .first_mut()
            .expect("at least one owned account")
            .public_key(),
    );

    let provider = Provider::new(
        runtime::Handle::current(),
        Box::new(NoopLogger::<L1ChainSpec>::default()),
        Box::new(|_| {}),
        config,
        Arc::new(RwLock::new(decoder)),
        CurrentTime,
    )?;

    Ok((provider, from))
}

fn creation_bytes(
    output: &CompilerOutput<SolxBytecode>,
    file: &str,
    contract: &str,
) -> anyhow::Result<Bytes> {
    let evm = &output
        .contracts
        .get(file)
        .and_then(|m| m.get(contract))
        .with_context(|| format!("fixture missing {file}::{contract}"))?
        .evm;
    Ok(Bytes::from(hex::decode(&evm.bytecode.object)?))
}

fn selector(signature: &str) -> Selector {
    Selector::from_slice(&keccak256(signature.as_bytes())[..4])
}

fn deploy(
    provider: &Provider<L1ChainSpec>,
    from: Address,
    creation: Bytes,
) -> anyhow::Result<Address> {
    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SendTransaction(TransactionRequest {
            from,
            data: Some(creation),
            ..TransactionRequest::default()
        }),
    ))?;
    let tx_hash: B256 = serde_json::from_value(response.result)?;
    let receipt_response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetTransactionReceipt(tx_hash),
    ))?;
    let receipt: L1RpcTransactionReceipt = serde_json::from_value(receipt_response.result)?;
    receipt
        .contract_address
        .context("deployment receipt must carry contract_address")
}

/// Sends a transaction and expects [`ProviderError::TransactionFailed`] to
/// be returned — i.e. the call reverted under `bail_on_transaction_failure`.
/// Pulls the stack trace out of the failure and returns it directly to
/// avoid naming `TransactionFailureWithCallTraces` (its module is private).
fn expect_failed_call_stack_trace(
    provider: &Provider<L1ChainSpec>,
    from: Address,
    to: Address,
    calldata: Bytes,
) -> Vec<StackTraceEntry> {
    let err = provider
        .handle_request(ProviderRequest::with_single(
            MethodInvocation::SendTransaction(TransactionRequest {
                from,
                to: Some(to),
                data: Some(calldata),
                ..TransactionRequest::default()
            }),
        ))
        .expect_err("call must revert and bail");
    match err {
        ProviderError::TransactionFailed(boxed) => match &boxed.failure.stack_trace_result {
            StackTraceCreationResult::Success(v) => v.clone(),
            other => panic!("expected StackTraceCreationResult::Success, got {other:?}"),
        },
        other => panic!("expected TransactionFailed, got: {other:?}"),
    }
}

fn source_reference_of(entry: &StackTraceEntry) -> Option<&SourceReference> {
    match entry {
        StackTraceEntry::CallstackEntry {
            source_reference, ..
        }
        | StackTraceEntry::RevertError {
            source_reference, ..
        }
        | StackTraceEntry::CheatCodeError {
            source_reference, ..
        }
        | StackTraceEntry::CustomError {
            source_reference, ..
        }
        | StackTraceEntry::FunctionNotPayableError {
            source_reference, ..
        }
        | StackTraceEntry::InvalidParamsError { source_reference }
        | StackTraceEntry::FallbackNotPayableError {
            source_reference, ..
        }
        | StackTraceEntry::FallbackNotPayableAndNoReceiveError {
            source_reference, ..
        }
        | StackTraceEntry::UnrecognizedFunctionWithoutFallbackError { source_reference }
        | StackTraceEntry::MissingFallbackOrReceiveError { source_reference }
        | StackTraceEntry::ReturndataSizeError { source_reference }
        | StackTraceEntry::NoncontractAccountCalledError { source_reference }
        | StackTraceEntry::CallFailedError { source_reference }
        | StackTraceEntry::DirectLibraryCallError { source_reference }
        | StackTraceEntry::InternalFunctionCallstackEntry {
            source_reference, ..
        } => Some(source_reference),
        StackTraceEntry::PanicError {
            source_reference, ..
        }
        | StackTraceEntry::OtherExecutionError { source_reference }
        | StackTraceEntry::UnmappedSolc0_6_3RevertError { source_reference }
        | StackTraceEntry::ContractTooLargeError { source_reference }
        | StackTraceEntry::ContractCallRunOutOfGasError { source_reference } => {
            source_reference.as_ref()
        }
        StackTraceEntry::UnrecognizedCreateCallstackEntry
        | StackTraceEntry::UnrecognizedContractCallstackEntry { .. }
        | StackTraceEntry::PrecompileError { .. }
        | StackTraceEntry::UnrecognizedCreateError { .. }
        | StackTraceEntry::UnrecognizedContractError { .. } => None,
    }
}

// ---------- variance-axis tests ----------

/// Counter.set(0) reverts via `require(v > 0, "must be positive")`.
/// Pin: stack trace surfaces a [`StackTraceEntry::RevertError`] referencing
/// Counter.sol. Covers the provider-flow plumbing end-to-end and
/// the `RevertError` axis.
#[tokio::test(flavor = "multi_thread")]
async fn revert_error_variant_surfaces_for_counter() -> anyhow::Result<()> {
    let (build_info, output) = solx_counter_build_info()?;
    let decoder = ContractDecoder::new(&build_info)?;
    let (provider, from) = make_provider(decoder)?;

    let counter = deploy(
        &provider,
        from,
        creation_bytes(&output, "Counter.sol", "Counter")?,
    )?;

    let mut calldata = Vec::with_capacity(36);
    calldata.extend_from_slice(selector("set(uint256)").as_slice());
    calldata.extend_from_slice(&[0u8; 32]);
    let stack_trace =
        expect_failed_call_stack_trace(&provider, from, counter, Bytes::from(calldata));

    assert!(
        stack_trace
            .iter()
            .any(|e| matches!(e, StackTraceEntry::RevertError { .. })),
        "expected a RevertError entry, got: {stack_trace:#?}"
    );
    assert!(
        stack_trace.iter().any(|e| source_reference_of(e)
            .is_some_and(|s| s.source_name.ends_with("Counter.sol"))),
        "expected an entry referencing Counter.sol, got: {stack_trace:#?}"
    );
    Ok(())
}

/// OverflowTest.testOverflow does `x = x + 1` with `x = uint256.max` →
/// panic 0x11. Pin: stack trace surfaces a [`StackTraceEntry::PanicError`].
/// Covers the `PanicError` axis.
#[tokio::test(flavor = "multi_thread")]
async fn panic_error_variant_surfaces_for_overflow_scenario() -> anyhow::Result<()> {
    let (build_info, output) = solx_scenarios_build_info()?;
    let decoder = ContractDecoder::new(&build_info)?;
    let (provider, from) = make_provider(decoder)?;

    let addr = deploy(
        &provider,
        from,
        creation_bytes(&output, "project/contracts/Scenarios.t.sol", "OverflowTest")?,
    )?;

    let stack_trace = expect_failed_call_stack_trace(
        &provider,
        from,
        addr,
        Bytes::from(selector("testOverflow()").as_slice().to_vec()),
    );

    assert!(
        stack_trace
            .iter()
            .any(|e| matches!(e, StackTraceEntry::PanicError { .. })),
        "expected a PanicError entry, got: {stack_trace:#?}"
    );
    Ok(())
}

/// CustomErrorTest.testCustomError does `revert MyError(42, "...")`.
/// Pin: stack trace surfaces a [`StackTraceEntry::CustomError`].
/// Covers the `CustomError` axis.
#[tokio::test(flavor = "multi_thread")]
async fn custom_error_variant_surfaces_for_custom_error_scenario() -> anyhow::Result<()> {
    let (build_info, output) = solx_scenarios_build_info()?;
    let decoder = ContractDecoder::new(&build_info)?;
    let (provider, from) = make_provider(decoder)?;

    let addr = deploy(
        &provider,
        from,
        creation_bytes(
            &output,
            "project/contracts/Scenarios.t.sol",
            "CustomErrorTest",
        )?,
    )?;

    let stack_trace = expect_failed_call_stack_trace(
        &provider,
        from,
        addr,
        Bytes::from(selector("testCustomError()").as_slice().to_vec()),
    );

    assert!(
        stack_trace
            .iter()
            .any(|e| matches!(e, StackTraceEntry::CustomError { .. })),
        "expected a CustomError entry, got: {stack_trace:#?}"
    );
    Ok(())
}
