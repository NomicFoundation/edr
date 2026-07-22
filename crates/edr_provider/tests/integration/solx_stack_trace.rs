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
use edr_primitives::{hex, keccak256, Address, Bytes, Selector, B256, U256};
use edr_provider::{
    test_utils::{create_test_config_with, MinimalProviderConfig},
    time::CurrentTime,
    MethodInvocation, NoopLogger, Provider, ProviderError, ProviderRequest,
};
use edr_signer::public_key_to_address;
use edr_solidity::{
    artifacts::{
        solx::extract_solx_contract_metadata, BuildInfoConfig, CompilerInput, CompilerOutput,
        SolxBytecode,
    },
    contract_decoder::ContractDecoder,
    solidity_stack_trace::{SourceReference, StackTraceCreationResult, StackTraceEntry},
};
use parking_lot::RwLock;
use tokio::runtime;

fn solx_counter_build_info() -> anyhow::Result<(BuildInfoConfig, CompilerOutput<SolxBytecode>)> {
    let mut input: CompilerInput = serde_json::from_str(include_str!(
        "../../../edr_solidity/fixtures/solx_compiler_input.json"
    ))?;
    input.sources.get_mut("Counter.sol").unwrap().content =
        include_str!("../../../edr_solidity/fixtures/sources/Counter.sol").to_owned();

    let output: CompilerOutput<SolxBytecode> = serde_json::from_str(include_str!(
        "../../../edr_solidity/fixtures/solx_compiler_output.json"
    ))?;

    let identified_contracts =
        extract_solx_contract_metadata("0.8.34".to_owned(), input, output.clone())?;

    Ok((
        BuildInfoConfig {
            identified_contracts,
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
        include_str!("../../../edr_solidity/fixtures/sources/Scenarios.t.sol").to_owned();

    let output: CompilerOutput<SolxBytecode> = serde_json::from_str(include_str!(
        "../../../edr_solidity/fixtures/solx_compiler_output_scenarios.json"
    ))?;

    let identified_contracts =
        extract_solx_contract_metadata("0.8.34".to_owned(), input, output.clone())?;

    Ok((
        BuildInfoConfig {
            identified_contracts,
            ignore_contracts: None,
        },
        output,
    ))
}

fn solx_stack_trace_scenarios_build_info(
) -> anyhow::Result<(BuildInfoConfig, CompilerOutput<SolxBytecode>)> {
    let mut input: CompilerInput = serde_json::from_str(include_str!(
        "../../../edr_solidity/fixtures/solx_compiler_input_stack_trace_scenarios.json"
    ))?;

    input
        .sources
        .get_mut("project/contracts/StackTraceScenarios.sol")
        .unwrap()
        .content =
        include_str!("../../../edr_solidity/fixtures/sources/StackTraceScenarios.sol").to_owned();

    let output: CompilerOutput<SolxBytecode> = serde_json::from_str(include_str!(
        "../../../edr_solidity/fixtures/solx_compiler_output_stack_trace_scenarios.json"
    ))?;

    let identified_contracts =
        extract_solx_contract_metadata("0.8.34".to_owned(), input, output.clone())?;

    Ok((
        BuildInfoConfig {
            identified_contracts,
            ignore_contracts: None,
        },
        output,
    ))
}

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
    let hash = keccak256(signature.as_bytes());
    Selector::from(
        *hash
            .first_chunk::<4>()
            .expect("keccak256 output is 32 bytes"),
    )
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
    expect_failed_call_with_value_stack_trace(provider, from, to, calldata, U256::ZERO)
}

/// Like [`expect_failed_call_stack_trace`], but transfers `value` — for the
/// payability dispatch errors.
fn expect_failed_call_with_value_stack_trace(
    provider: &Provider<L1ChainSpec>,
    from: Address,
    to: Address,
    calldata: Bytes,
    value: U256,
) -> Vec<StackTraceEntry> {
    let err = provider
        .handle_request(ProviderRequest::with_single(
            MethodInvocation::SendTransaction(TransactionRequest {
                from,
                to: Some(to),
                data: Some(calldata),
                value: Some(value),
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

fn encode_call_u256(signature: &str, v: u64) -> Bytes {
    let mut calldata = Vec::with_capacity(36);
    calldata.extend_from_slice(selector(signature).as_slice());
    calldata.extend_from_slice(&U256::from(v).to_be_bytes::<32>());
    Bytes::from(calldata)
}

fn encode_call_address(signature: &str, addr: Address) -> Bytes {
    let mut calldata = Vec::with_capacity(36);
    calldata.extend_from_slice(selector(signature).as_slice());
    calldata.extend_from_slice(&[0u8; 12]);
    calldata.extend_from_slice(addr.as_slice());
    Bytes::from(calldata)
}

/// Sends a transaction that must succeed (e.g. a scenario's `setUp()`).
fn send_ok(
    provider: &Provider<L1ChainSpec>,
    from: Address,
    to: Address,
    calldata: Bytes,
) -> anyhow::Result<()> {
    provider
        .handle_request(ProviderRequest::with_single(
            MethodInvocation::SendTransaction(TransactionRequest {
                from,
                to: Some(to),
                data: Some(calldata),
                ..TransactionRequest::default()
            }),
        ))
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!("setup transaction must succeed: {e:?}"))
}

/// Sends a deployment transaction and expects it to revert during CREATE,
/// returning the stack trace of the failed deployment.
fn expect_failed_deploy_stack_trace(
    provider: &Provider<L1ChainSpec>,
    from: Address,
    creation: Bytes,
) -> Vec<StackTraceEntry> {
    let err = provider
        .handle_request(ProviderRequest::with_single(
            MethodInvocation::SendTransaction(TransactionRequest {
                from,
                data: Some(creation),
                ..TransactionRequest::default()
            }),
        ))
        .expect_err("deployment must revert and bail");
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
    let decoder = ContractDecoder::new(build_info);
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
    let decoder = ContractDecoder::new(build_info);
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
    let decoder = ContractDecoder::new(build_info);
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

// ---------- scenarios-fixture breadth tests ----------
//
// One test per DWARF-decoding axis the JS parity sweep can't pin at the
// provider level. Assertions stay at "entry variant + source line": exact
// frame-shape parity with solc remains the sweep's job.

const SCENARIOS_SOURCE: &str = "project/contracts/Scenarios.t.sol";

fn scenarios_provider(
) -> anyhow::Result<(Provider<L1ChainSpec>, Address, CompilerOutput<SolxBytecode>)> {
    let (build_info, output) = solx_scenarios_build_info()?;
    let decoder = ContractDecoder::new(build_info);
    let (provider, from) = make_provider(decoder)?;
    Ok((provider, from, output))
}

fn deploy_scenario(
    provider: &Provider<L1ChainSpec>,
    from: Address,
    output: &CompilerOutput<SolxBytecode>,
    contract: &str,
) -> anyhow::Result<Address> {
    deploy(
        provider,
        from,
        creation_bytes(output, SCENARIOS_SOURCE, contract)?,
    )
}

fn contains_ascii(data: &[u8], needle: &str) -> bool {
    data.windows(needle.len()).any(|w| w == needle.as_bytes())
}

/// One line per entry, without the embedded `source_content` that makes
/// `{:#?}` dumps of [`SourceReference`] unreadable.
fn brief_trace(stack_trace: &[StackTraceEntry]) -> String {
    stack_trace
        .iter()
        .map(|entry| {
            let debug = format!("{entry:?}");
            let variant = debug.split_whitespace().next().unwrap_or("?").to_string();
            match source_reference_of(entry) {
                Some(source_reference) => format!(
                    "{variant} {}:{} ({}.{})",
                    source_reference.source_name,
                    source_reference.line,
                    source_reference.contract.as_deref().unwrap_or("?"),
                    source_reference.function.as_deref().unwrap_or("?"),
                ),
                None => variant,
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Finds the sole `RevertError` entry and asserts its source line and
/// reason string (the reason is ABI-encoded inside `return_data`).
#[track_caller]
fn assert_revert_at_line(stack_trace: &[StackTraceEntry], line: u32, reason: &str) {
    let (return_data, source_reference) = stack_trace
        .iter()
        .find_map(|e| match e {
            StackTraceEntry::RevertError {
                return_data,
                source_reference,
                ..
            } => Some((return_data, source_reference)),
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "expected a RevertError entry, got:\n{}",
                brief_trace(stack_trace)
            )
        });
    assert!(
        contains_ascii(return_data, reason),
        "expected revert reason {reason:?} in return data, got: {return_data:?}"
    );
    assert_eq!(
        source_reference.line,
        line,
        "expected RevertError at {}:{line}, got:\n{}",
        source_reference.source_name,
        brief_trace(stack_trace)
    );
}

/// Deploys a scenario contract, triggers `calldata`, and asserts the trace
/// pins a `RevertError` to `line` with `reason`.
fn expect_scenario_revert(
    contract: &str,
    calldata: Bytes,
    line: u32,
    reason: &str,
) -> anyhow::Result<Vec<StackTraceEntry>> {
    let (provider, from, output) = scenarios_provider()?;
    let addr = deploy_scenario(&provider, from, &output, contract)?;
    let stack_trace = expect_failed_call_stack_trace(&provider, from, addr, calldata);
    assert_revert_at_line(&stack_trace, line, reason);
    Ok(stack_trace)
}

/// Deploys a scenario contract, triggers `signature`, and asserts the trace
/// surfaces a `PanicError` with `code`.
fn expect_scenario_panic(contract: &str, signature: &str, code: u64) -> anyhow::Result<()> {
    let (provider, from, output) = scenarios_provider()?;
    let addr = deploy_scenario(&provider, from, &output, contract)?;
    let stack_trace = expect_failed_call_stack_trace(
        &provider,
        from,
        addr,
        Bytes::from(selector(signature).as_slice().to_vec()),
    );
    let error_code = stack_trace
        .iter()
        .find_map(|e| match e {
            StackTraceEntry::PanicError { error_code, .. } => Some(*error_code),
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "expected a PanicError entry, got:\n{}",
                brief_trace(&stack_trace)
            )
        });
    assert_eq!(
        error_code,
        U256::from(code),
        "expected panic code {code:#x}, got:\n{}",
        brief_trace(&stack_trace)
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn panic_code_surfaces_for_assert_failure() -> anyhow::Result<()> {
    expect_scenario_panic("AssertionFailureTest", "testAssertionFails()", 0x01)
}

#[tokio::test(flavor = "multi_thread")]
async fn panic_code_surfaces_for_division_by_zero() -> anyhow::Result<()> {
    expect_scenario_panic("DivisionByZeroTest", "testDivisionByZero()", 0x12)
}

#[tokio::test(flavor = "multi_thread")]
async fn panic_code_surfaces_for_invalid_enum_cast() -> anyhow::Result<()> {
    expect_scenario_panic("InvalidEnumCastTest", "testInvalidEnumCast()", 0x21)
}

#[tokio::test(flavor = "multi_thread")]
async fn panic_code_surfaces_for_pop_on_empty_array() -> anyhow::Result<()> {
    expect_scenario_panic("PopEmptyArrayTest", "testPopEmpty()", 0x31)
}

#[tokio::test(flavor = "multi_thread")]
async fn panic_code_surfaces_for_array_out_of_bounds() -> anyhow::Result<()> {
    expect_scenario_panic("ArrayOutOfBoundsTest", "testArrayOOB()", 0x32)
}

/// Two `require`s in one function; only the second fails. Pin: the
/// `RevertError` line discriminates between statements — the sharpest
/// DWARF line-attribution probe available per scenario.
#[tokio::test(flavor = "multi_thread")]
async fn revert_line_discriminates_between_requires() -> anyhow::Result<()> {
    expect_scenario_revert(
        "MultipleRequiresTest",
        Bytes::from(selector("testMultipleRequires()").as_slice().to_vec()),
        340,
        "second",
    )?;
    Ok(())
}

/// CREATE-path decoding: the constructor itself reverts, so the trace must
/// resolve through `evm.bytecode.debugInfo` (creation code), not
/// `evm.deployedBytecode.debugInfo`.
#[tokio::test(flavor = "multi_thread")]
async fn create_revert_surfaces_for_reverting_constructor() -> anyhow::Result<()> {
    let (provider, from, output) = scenarios_provider()?;
    let stack_trace = expect_failed_deploy_stack_trace(
        &provider,
        from,
        creation_bytes(&output, SCENARIOS_SOURCE, "ConstructorRevertContract")?,
    );
    assert_revert_at_line(&stack_trace, 57, "constructor boom");
    Ok(())
}

/// CREATE-path decoding with an internal helper frame: constructor calls
/// `_check(v)` which reverts.
#[tokio::test(flavor = "multi_thread")]
async fn create_revert_surfaces_through_constructor_helper() -> anyhow::Result<()> {
    let (provider, from, output) = scenarios_provider()?;
    let mut creation = creation_bytes(
        &output,
        SCENARIOS_SOURCE,
        "HelperRevertingConstructorContract",
    )?
    .to_vec();
    creation.extend_from_slice(&[0u8; 32]); // constructor(uint256 v = 0)
    let stack_trace = expect_failed_deploy_stack_trace(&provider, from, Bytes::from(creation));
    assert_revert_at_line(&stack_trace, 295, "constructor helper boom");
    Ok(())
}

/// Revert inside a modifier body (`onlyPositive`).
#[tokio::test(flavor = "multi_thread")]
async fn modifier_revert_points_at_modifier_require() -> anyhow::Result<()> {
    expect_scenario_revert(
        "ModifierTarget",
        encode_call_u256("setIfPositive(uint256)", 0),
        87,
        "modifier must be positive",
    )?;
    Ok(())
}

/// Revert inside a multi-statement modifier body (`validates`, pre-`_`).
///
/// solx flattens the modifier into `bumpIfValid` and — in this fixture's
/// pre-0.1.6 artifacts (not regenerable, see `solx_fixtures.rs`) — attributes
/// the shared revert helper to the function declaration line (420);
/// `SolxTraceStrategy::revert_source_reference` walks the executed steps
/// back to the message-building code of the `require` that actually fired
/// (line 415), matching solc. The same shape on current artifacts is
/// pinned in `nested_modifier_revert_walks_back_from_line_zero_helper`.
#[tokio::test(flavor = "multi_thread")]
async fn nested_modifier_revert_points_at_failing_require() -> anyhow::Result<()> {
    expect_scenario_revert(
        "NestedModifierTarget",
        encode_call_u256("bumpIfValid(uint256)", 13),
        415,
        "unlucky",
    )?;
    Ok(())
}

/// Cross-contract CALL chain: `CrossContractCallTest.testCrossContractCall`
/// calls `Other.fail()`. Pin: the revert resolves inside `Other` and the
/// trace keeps a callstack frame for the calling contract.
#[tokio::test(flavor = "multi_thread")]
async fn cross_contract_call_keeps_caller_frame() -> anyhow::Result<()> {
    let (provider, from, output) = scenarios_provider()?;
    let caller = deploy_scenario(&provider, from, &output, "CrossContractCallTest")?;
    send_ok(
        &provider,
        from,
        caller,
        Bytes::from(selector("setUp()").as_slice().to_vec()),
    )?;
    let stack_trace = expect_failed_call_stack_trace(
        &provider,
        from,
        caller,
        Bytes::from(selector("testCrossContractCall()").as_slice().to_vec()),
    );
    assert_revert_at_line(&stack_trace, 69, "called fail");
    assert!(
        stack_trace.iter().any(|e| matches!(
            e,
            StackTraceEntry::CallstackEntry { source_reference, .. }
                if source_reference.contract.as_deref() == Some("CrossContractCallTest")
        )),
        "expected a CallstackEntry for CrossContractCallTest, got:\n{}",
        brief_trace(&stack_trace)
    );
    Ok(())
}

/// External self-recursion: `recurse(3)` re-enters via `this.recurse` three
/// times before reverting. Pin: one caller frame per CALL plus the bottom
/// revert.
#[tokio::test(flavor = "multi_thread")]
async fn external_recursion_keeps_one_frame_per_call() -> anyhow::Result<()> {
    let (provider, from, output) = scenarios_provider()?;
    let addr = deploy_scenario(&provider, from, &output, "DeepRecursionTarget")?;
    let stack_trace = expect_failed_call_stack_trace(
        &provider,
        from,
        addr,
        encode_call_u256("recurse(uint256)", 3),
    );
    assert_revert_at_line(&stack_trace, 109, "bottomed out");
    let recursion_frames = stack_trace
        .iter()
        .filter(|e| {
            matches!(
                e,
                StackTraceEntry::CallstackEntry { source_reference, .. }
                    if source_reference.function.as_deref() == Some("recurse")
            )
        })
        .count();
    assert!(
        recursion_frames >= 3,
        "expected >= 3 recurse callstack frames, got {recursion_frames}:\n{}",
        brief_trace(&stack_trace)
    );
    Ok(())
}

/// Internal (same-frame) recursion; solx's optimizer may unroll it, so only
/// the bottom revert line is pinned — frame shape is the sweep's job.
#[tokio::test(flavor = "multi_thread")]
async fn internal_recursion_pins_bottom_revert_line() -> anyhow::Result<()> {
    expect_scenario_revert(
        "InternalRecurseTest",
        Bytes::from(selector("testInternalRecurse()").as_slice().to_vec()),
        348,
        "internal bottom",
    )?;
    Ok(())
}

/// Internal helper chain: `set(v)` -> `_checkPositive(v)` -> revert.
#[tokio::test(flavor = "multi_thread")]
async fn internal_helper_revert_points_at_helper_require() -> anyhow::Result<()> {
    expect_scenario_revert(
        "InternalHelperChainContract",
        encode_call_u256("set(uint256)", 0),
        149,
        "must be positive",
    )?;
    Ok(())
}

/// Internal library function revert (`RevertingLib.alwaysReverts`, inlined).
#[tokio::test(flavor = "multi_thread")]
async fn internal_library_revert_points_into_library() -> anyhow::Result<()> {
    expect_scenario_revert(
        "LibraryRevertTest",
        Bytes::from(selector("testLibraryRevert()").as_slice().to_vec()),
        229,
        "lib boom",
    )?;
    Ok(())
}

/// Revert inside `fallback()`, reached via an unknown selector.
#[tokio::test(flavor = "multi_thread")]
async fn fallback_revert_points_at_fallback_body() -> anyhow::Result<()> {
    expect_scenario_revert(
        "FallbackRevertTarget",
        Bytes::from(selector("nonExistent()").as_slice().to_vec()),
        259,
        "fallback boom",
    )?;
    Ok(())
}

/// Revert inside `receive()`, reached via empty calldata.
#[tokio::test(flavor = "multi_thread")]
async fn receive_revert_points_at_receive_body() -> anyhow::Result<()> {
    expect_scenario_revert("ReceiveRevertTarget", Bytes::new(), 276, "receive boom")?;
    Ok(())
}

/// Mutual recursion across CALL boundaries (`MutualA.pingA` <->
/// `MutualB.pingB`). solx may emit both JUMP-derived and inlined frames at
/// the dispatch points, so only the bottom revert line is pinned.
#[tokio::test(flavor = "multi_thread")]
async fn mutual_recursion_pins_bottom_revert_line() -> anyhow::Result<()> {
    let (provider, from, output) = scenarios_provider()?;
    let a = deploy_scenario(&provider, from, &output, "MutualA")?;
    let b = deploy_scenario(&provider, from, &output, "MutualB")?;
    send_ok(
        &provider,
        from,
        a,
        encode_call_address("setOther(address)", b),
    )?;
    send_ok(
        &provider,
        from,
        b,
        encode_call_address("setOther(address)", a),
    )?;
    let stack_trace =
        expect_failed_call_stack_trace(&provider, from, a, encode_call_u256("pingA(uint256)", 2));
    assert_revert_at_line(&stack_trace, 377, "mutual bottom");
    Ok(())
}

// ---------- StackTraceScenarios fixture tests ----------
//
// Dispatch-level and call-shape errors from the StackTraceScenarios fixture
// — entry variants the solc corpus covers that no solx test reached.
// Regenerate the fixture with `cargo run -p edr_tool_cli -- gen-solx-fixtures`
// when bumping solx.

const STACK_TRACE_SCENARIOS_SOURCE: &str = "project/contracts/StackTraceScenarios.sol";

fn stack_trace_scenarios_provider(
) -> anyhow::Result<(Provider<L1ChainSpec>, Address, CompilerOutput<SolxBytecode>)> {
    let (build_info, output) = solx_stack_trace_scenarios_build_info()?;
    let decoder = ContractDecoder::new(build_info);
    let (provider, from) = make_provider(decoder)?;
    Ok((provider, from, output))
}

fn deploy_stack_trace_scenario(
    provider: &Provider<L1ChainSpec>,
    from: Address,
    output: &CompilerOutput<SolxBytecode>,
    contract: &str,
) -> anyhow::Result<Address> {
    deploy(
        provider,
        from,
        creation_bytes(output, STACK_TRACE_SCENARIOS_SOURCE, contract)?,
    )
}

/// Replaces solc-style `__$<keccak>$__` library placeholders (40 chars, the
/// width of a hex-encoded address) with the deployed library address.
/// Bails on bytecode referencing more than one distinct library — this
/// helper takes a single address.
fn link_library(bytecode_object: &str, library: Address) -> anyhow::Result<String> {
    let address_hex = hex::encode(library);
    let mut linked = String::with_capacity(bytecode_object.len());
    let mut placeholder: Option<&str> = None;
    let mut rest = bytecode_object;
    while let Some(start) = rest.find("__$") {
        let placeholder_end = start + 40;
        anyhow::ensure!(
            rest.len() >= placeholder_end && rest[..placeholder_end].ends_with("$__"),
            "malformed link placeholder in bytecode"
        );
        let found = &rest[start..placeholder_end];
        anyhow::ensure!(
            placeholder.is_none_or(|first| first == found),
            "bytecode references multiple libraries; link_library takes one address"
        );
        placeholder = Some(found);
        linked.push_str(&rest[..start]);
        linked.push_str(&address_hex);
        rest = &rest[placeholder_end..];
    }
    linked.push_str(rest);
    Ok(linked)
}

#[track_caller]
fn assert_single_variant<'a>(
    stack_trace: &'a [StackTraceEntry],
    matcher: impl Fn(&StackTraceEntry) -> bool,
    variant: &str,
) -> &'a StackTraceEntry {
    stack_trace.iter().find(|e| matcher(e)).unwrap_or_else(|| {
        panic!(
            "expected a {variant} entry, got:\n{}",
            brief_trace(stack_trace)
        )
    })
}

/// Sending value to a non-payable function.
#[tokio::test(flavor = "multi_thread")]
async fn function_not_payable_error_surfaces() -> anyhow::Result<()> {
    let (provider, from, output) = stack_trace_scenarios_provider()?;
    let addr = deploy_stack_trace_scenario(&provider, from, &output, "NotPayable")?;
    let stack_trace = expect_failed_call_with_value_stack_trace(
        &provider,
        from,
        addr,
        encode_call_u256("store(uint256)", 1),
        U256::from(1u64),
    );
    let entry = assert_single_variant(
        &stack_trace,
        |e| matches!(e, StackTraceEntry::FunctionNotPayableError { .. }),
        "FunctionNotPayableError",
    );
    let source_reference =
        source_reference_of(entry).expect("FunctionNotPayableError carries a source reference");
    assert_eq!(source_reference.function.as_deref(), Some("store"));
    Ok(())
}

/// Calling an unknown selector on a contract without a fallback.
#[tokio::test(flavor = "multi_thread")]
async fn unrecognized_function_without_fallback_error_surfaces() -> anyhow::Result<()> {
    let (provider, from, output) = stack_trace_scenarios_provider()?;
    let addr = deploy_stack_trace_scenario(&provider, from, &output, "NoFallback")?;
    let stack_trace = expect_failed_call_stack_trace(
        &provider,
        from,
        addr,
        Bytes::from(selector("nonExistent()").as_slice().to_vec()),
    );
    assert_single_variant(
        &stack_trace,
        |e| {
            matches!(
                e,
                StackTraceEntry::UnrecognizedFunctionWithoutFallbackError { .. }
            )
        },
        "UnrecognizedFunctionWithoutFallbackError",
    );
    Ok(())
}

/// Plain value transfer to a contract with neither fallback nor receive.
#[tokio::test(flavor = "multi_thread")]
async fn missing_fallback_or_receive_error_surfaces() -> anyhow::Result<()> {
    let (provider, from, output) = stack_trace_scenarios_provider()?;
    let addr = deploy_stack_trace_scenario(&provider, from, &output, "NoFallback")?;
    let stack_trace = expect_failed_call_with_value_stack_trace(
        &provider,
        from,
        addr,
        Bytes::new(),
        U256::from(1u64),
    );
    assert_single_variant(
        &stack_trace,
        |e| matches!(e, StackTraceEntry::MissingFallbackOrReceiveError { .. }),
        "MissingFallbackOrReceiveError",
    );
    Ok(())
}

/// Value + calldata hitting a non-payable fallback.
#[tokio::test(flavor = "multi_thread")]
async fn fallback_not_payable_error_surfaces() -> anyhow::Result<()> {
    let (provider, from, output) = stack_trace_scenarios_provider()?;
    let addr = deploy_stack_trace_scenario(&provider, from, &output, "NonPayableFallback")?;
    let stack_trace = expect_failed_call_with_value_stack_trace(
        &provider,
        from,
        addr,
        Bytes::from(selector("nonExistent()").as_slice().to_vec()),
        U256::from(1u64),
    );
    assert_single_variant(
        &stack_trace,
        |e| matches!(e, StackTraceEntry::FallbackNotPayableError { .. }),
        "FallbackNotPayableError",
    );
    Ok(())
}

/// Plain value transfer where only a non-payable fallback exists.
#[tokio::test(flavor = "multi_thread")]
async fn fallback_not_payable_and_no_receive_error_surfaces() -> anyhow::Result<()> {
    let (provider, from, output) = stack_trace_scenarios_provider()?;
    let addr = deploy_stack_trace_scenario(&provider, from, &output, "NonPayableFallback")?;
    let stack_trace = expect_failed_call_with_value_stack_trace(
        &provider,
        from,
        addr,
        Bytes::new(),
        U256::from(1u64),
    );
    assert_single_variant(
        &stack_trace,
        |e| {
            matches!(
                e,
                StackTraceEntry::FallbackNotPayableAndNoReceiveError { .. }
            )
        },
        "FallbackNotPayableAndNoReceiveError",
    );
    Ok(())
}

/// Truncated calldata: right selector, missing argument words.
#[tokio::test(flavor = "multi_thread")]
async fn invalid_params_error_surfaces_for_truncated_calldata() -> anyhow::Result<()> {
    let (provider, from, output) = stack_trace_scenarios_provider()?;
    let addr = deploy_stack_trace_scenario(&provider, from, &output, "RequiresArgs")?;
    let mut calldata = selector("needsBoth(uint256,uint256)").as_slice().to_vec();
    calldata.extend_from_slice(&[0u8; 32]); // only one of the two words
    let stack_trace = expect_failed_call_stack_trace(&provider, from, addr, Bytes::from(calldata));
    assert_single_variant(
        &stack_trace,
        |e| matches!(e, StackTraceEntry::InvalidParamsError { .. }),
        "InvalidParamsError",
    );
    Ok(())
}

#[track_caller]
fn assert_returndata_size_error_at_call_get(stack_trace: &[StackTraceEntry]) {
    let entry = assert_single_variant(
        stack_trace,
        |e| matches!(e, StackTraceEntry::ReturndataSizeError { .. }),
        "ReturndataSizeError",
    );
    let source_reference =
        source_reference_of(entry).expect("ReturndataSizeError carries a source reference");
    assert_eq!(source_reference.function.as_deref(), Some("callGet"));
    assert_eq!(
        source_reference.line,
        47,
        "expected the call-site line, got:\n{}",
        brief_trace(stack_trace)
    );
}

/// Interface promises a word; callee returns nothing. Pin: the
/// returndata-size check reverting right after the call is attributed to
/// the call site, matching the solc route.
#[tokio::test(flavor = "multi_thread")]
async fn returndata_size_error_surfaces_at_call_site() -> anyhow::Result<()> {
    let (provider, from, output) = stack_trace_scenarios_provider()?;
    let callee = deploy_stack_trace_scenario(&provider, from, &output, "ReturnsNothing")?;
    let caller = deploy_stack_trace_scenario(&provider, from, &output, "ExpectsWord")?;
    let stack_trace = expect_failed_call_stack_trace(
        &provider,
        from,
        caller,
        encode_call_address("callGet(address)", callee),
    );
    assert_returndata_size_error_at_call_get(&stack_trace);
    Ok(())
}

/// Typed call to an address with no code. Like solc (which emits no
/// EXTCODESIZE probe for returndata-expecting calls since 0.8.10), solx
/// surfaces this as the returndata-size check failing at the call site —
/// a true `NoncontractAccountCalledError` would need a void-returning
/// call scenario.
#[tokio::test(flavor = "multi_thread")]
async fn noncontract_account_call_surfaces_as_returndata_size_error() -> anyhow::Result<()> {
    let (provider, from, output) = stack_trace_scenarios_provider()?;
    let caller = deploy_stack_trace_scenario(&provider, from, &output, "ExpectsWord")?;
    let eoa = Address::repeat_byte(0x42);
    let stack_trace = expect_failed_call_stack_trace(
        &provider,
        from,
        caller,
        encode_call_address("callGet(address)", eoa),
    );
    assert_returndata_size_error_at_call_get(&stack_trace);
    Ok(())
}

/// External (public) library function reached through a linked contract:
/// exercises `linkReferences` placeholder substitution and DELEGATECALL
/// frame decoding into the library's own debugInfo.
#[tokio::test(flavor = "multi_thread")]
async fn linked_external_library_revert_points_into_library() -> anyhow::Result<()> {
    let (provider, from, output) = stack_trace_scenarios_provider()?;
    let library = deploy_stack_trace_scenario(&provider, from, &output, "ExternalLib")?;

    let unlinked = &output
        .contracts
        .get(STACK_TRACE_SCENARIOS_SOURCE)
        .and_then(|m| m.get("UsesExternalLib"))
        .context("fixture missing UsesExternalLib")?
        .evm
        .bytecode
        .object;
    let linked = link_library(unlinked, library)?;
    let user = deploy(&provider, from, Bytes::from(hex::decode(&linked)?))?;

    let stack_trace = expect_failed_call_stack_trace(
        &provider,
        from,
        user,
        Bytes::from(selector("go()").as_slice().to_vec()),
    );
    assert_revert_at_line(&stack_trace, 53, "external lib boom");
    Ok(())
}

/// Calling a deployed library's external function directly.
#[tokio::test(flavor = "multi_thread")]
async fn direct_library_call_error_surfaces() -> anyhow::Result<()> {
    let (provider, from, output) = stack_trace_scenarios_provider()?;
    let library = deploy_stack_trace_scenario(&provider, from, &output, "ExternalLib")?;
    let stack_trace = expect_failed_call_stack_trace(
        &provider,
        from,
        library,
        Bytes::from(selector("fail()").as_slice().to_vec()),
    );
    assert_single_variant(
        &stack_trace,
        |e| matches!(e, StackTraceEntry::DirectLibraryCallError { .. }),
        "DirectLibraryCallError",
    );
    Ok(())
}

/// Revert inside a multi-statement modifier body (`validates`, pre-`_`),
/// compiled with current solx. Since 0.1.6 solx emits DWARF line 0 for the
/// flattened shared revert helper (compiler-generated code) instead of the
/// modified function's declaration line; the decoder's Pass-3 fallback
/// turns that into the function's AST location, so the walk-back still
/// triggers on a declaration-attributed location and recovers the
/// message-building code of the `require` that actually fired (line 80),
/// matching solc. The raw declaration-line attribution of pre-0.1.6
/// artifacts is pinned in `nested_modifier_revert_points_at_failing_require`.
#[tokio::test(flavor = "multi_thread")]
async fn nested_modifier_revert_walks_back_from_line_zero_helper() -> anyhow::Result<()> {
    let (provider, from, output) = stack_trace_scenarios_provider()?;
    let addr = deploy_stack_trace_scenario(&provider, from, &output, "ValidatedCounter")?;
    let stack_trace = expect_failed_call_stack_trace(
        &provider,
        from,
        addr,
        encode_call_u256("bumpIfValid(uint256)", 13),
    );
    assert_revert_at_line(&stack_trace, 80, "unlucky");
    Ok(())
}

/// Cross-contract variant of the modifier revert: `ValidatedCounterCaller`
/// CALLs `ValidatedCounter.bumpIfValid`, which reverts in the flattened
/// `validates` modifier. Pin solc's frame shape: a callstack frame for the
/// called function (its declaration, line 86 — recovered from the line-0
/// dispatch call site via the decl-line fallback) between the caller frame
/// and the revert.
#[tokio::test(flavor = "multi_thread")]
async fn cross_contract_modifier_revert_keeps_called_function_frame() -> anyhow::Result<()> {
    let (provider, from, output) = stack_trace_scenarios_provider()?;
    let addr = deploy_stack_trace_scenario(&provider, from, &output, "ValidatedCounterCaller")?;
    let stack_trace = expect_failed_call_stack_trace(
        &provider,
        from,
        addr,
        encode_call_u256("callBump(uint256)", 13),
    );
    assert_revert_at_line(&stack_trace, 80, "unlucky");
    let callee_frame = stack_trace.iter().any(|e| {
        matches!(e, StackTraceEntry::CallstackEntry { source_reference, .. }
            if source_reference.function.as_deref() == Some("bumpIfValid")
                && source_reference.line == 86)
    });
    assert!(
        callee_frame,
        "expected a CallstackEntry for bumpIfValid at its declaration \
         (line 86), got:\n{}",
        brief_trace(&stack_trace)
    );
    Ok(())
}

/// A modifier's bare `revert()` builds no message and its shared helper is
/// unmapped in the DWARF, so the selector-resolved fallback plus the
/// walk-back attribute the revert to the guard condition (line 67) inside
/// the modifier — the closest mapped statement. The solc route reports the
/// `revert()` statement itself (line 68); closing that last line is solx
/// line-table fidelity, not inferrable EDR-side.
#[tokio::test(flavor = "multi_thread")]
async fn bare_modifier_revert_attributes_to_the_guard() -> anyhow::Result<()> {
    let (provider, from, output) = stack_trace_scenarios_provider()?;
    let addr = deploy_stack_trace_scenario(&provider, from, &output, "GuardedBareRevert")?;
    let stack_trace = expect_failed_call_stack_trace(
        &provider,
        from,
        addr,
        Bytes::from(selector("fire()").as_slice().to_vec()),
    );
    let entry = assert_single_variant(
        &stack_trace,
        |e| matches!(e, StackTraceEntry::RevertError { .. }),
        "RevertError",
    );
    let source_reference = source_reference_of(entry).expect("entry carries a source reference");
    assert_eq!(
        (source_reference.line, source_reference.function.as_deref()),
        (67, Some("guarded")),
        "expected the guard line inside the modifier, got:\n{}",
        brief_trace(&stack_trace)
    );
    Ok(())
}
