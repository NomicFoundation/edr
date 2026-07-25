#![cfg(feature = "test-utils")]

//! Verifies the solx (DWARF) stack-trace path through the JSON-RPC
//! provider. Solidity-test runs are exercised by the JS parity sweep in
//! `js/integration-tests/solx-parity-sweep`; this file pins the
//! provider-side surface and exercises a small slice of the
//! [`StackTraceEntry`] variants we can hit from the existing solx fixtures.

use std::sync::Arc;

use anyhow::Context;
use edr_chain_l1::{rpc::TransactionRequest, L1ChainSpec};
use edr_primitives::{hex, keccak256, Address, Bytes, Selector, U256};
use edr_provider::{
    test_utils::{create_test_config_with, deploy_contract, MinimalProviderConfig},
    time::CurrentTime,
    MethodInvocation, NoopLogger, Provider, ProviderError, ProviderErrorForChainSpec,
    ProviderRequest,
};
use edr_signer::public_key_to_address;
use edr_solidity::{
    artifacts::{
        solx::extract_solx_contract_metadata, BuildInfoConfig, CompilerInput, CompilerOutput,
        SolxBytecode,
    },
    contract_decoder::ContractDecoder,
    solidity_stack_trace::{StackTraceCreationResult, StackTraceEntry},
};
use parking_lot::RwLock;
use tokio::runtime;

/// The `include_str!` literals stay at the call sites — the macro needs a
/// literal path.
fn assemble_build_info(
    mut input: CompilerInput,
    source_key: &str,
    source_content: &str,
    output: CompilerOutput<SolxBytecode>,
) -> anyhow::Result<(BuildInfoConfig, CompilerOutput<SolxBytecode>)> {
    input.sources.get_mut(source_key).unwrap().content = source_content.to_owned();

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

fn solx_counter_build_info() -> anyhow::Result<(BuildInfoConfig, CompilerOutput<SolxBytecode>)> {
    assemble_build_info(
        serde_json::from_str(include_str!(
            "../../../edr_solidity/fixtures/solx_compiler_input.json"
        ))?,
        "Counter.sol",
        include_str!("../../../edr_solidity/fixtures/sources/Counter.sol"),
        serde_json::from_str(include_str!(
            "../../../edr_solidity/fixtures/solx_compiler_output.json"
        ))?,
    )
}

fn solx_scenarios_build_info() -> anyhow::Result<(BuildInfoConfig, CompilerOutput<SolxBytecode>)> {
    assemble_build_info(
        serde_json::from_str(include_str!(
            "../../../edr_solidity/fixtures/solx_compiler_input_scenarios.json"
        ))?,
        SCENARIOS_SOURCE,
        include_str!("../../../edr_solidity/fixtures/sources/Scenarios.t.sol"),
        serde_json::from_str(include_str!(
            "../../../edr_solidity/fixtures/solx_compiler_output_scenarios.json"
        ))?,
    )
}

fn solx_stack_trace_scenarios_build_info(
    input_json: &str,
    output_json: &str,
) -> anyhow::Result<(BuildInfoConfig, CompilerOutput<SolxBytecode>)> {
    assemble_build_info(
        serde_json::from_str(input_json)?,
        STACK_TRACE_SCENARIOS_SOURCE,
        include_str!("../../../edr_solidity/fixtures/sources/StackTraceScenarios.sol"),
        serde_json::from_str(output_json)?,
    )
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

/// Pulls the stack trace out of the failure and returns it directly to
/// avoid naming `TransactionFailureWithCallTraces` (its module is private).
fn stack_trace_from_failure(err: ProviderErrorForChainSpec<L1ChainSpec>) -> Vec<StackTraceEntry> {
    match err {
        ProviderError::TransactionFailed(boxed) => match &boxed.failure.stack_trace_result {
            StackTraceCreationResult::Success(v) => v.clone(),
            other => panic!("expected StackTraceCreationResult::Success, got {other:?}"),
        },
        other => panic!("expected TransactionFailed, got: {other:?}"),
    }
}

/// Sends a transaction and expects [`ProviderError::TransactionFailed`] to
/// be returned — i.e. the call reverted under `bail_on_transaction_failure`.
fn expect_failed_call_stack_trace(
    provider: &Provider<L1ChainSpec>,
    from: Address,
    to: Address,
    calldata: Bytes,
) -> Vec<StackTraceEntry> {
    expect_failed_call_with_value_stack_trace(provider, from, to, calldata, U256::ZERO)
}

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
    stack_trace_from_failure(err)
}

/// No-argument calldata: just the 4-byte selector.
fn call(signature: &str) -> Bytes {
    Bytes::copy_from_slice(selector(signature).as_slice())
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

    let counter = deploy_contract(
        &provider,
        from,
        creation_bytes(&output, "Counter.sol", "Counter")?,
    )?;

    let stack_trace = expect_failed_call_stack_trace(
        &provider,
        from,
        counter,
        encode_call_u256("set(uint256)", 0),
    );

    assert!(
        stack_trace
            .iter()
            .any(|e| matches!(e, StackTraceEntry::RevertError { .. })),
        "expected a RevertError entry, got: {stack_trace:#?}"
    );
    assert!(
        stack_trace.iter().any(|e| e
            .source_reference()
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

    let addr = deploy_contract(
        &provider,
        from,
        creation_bytes(&output, "project/contracts/Scenarios.t.sol", "OverflowTest")?,
    )?;

    let stack_trace = expect_failed_call_stack_trace(&provider, from, addr, call("testOverflow()"));

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

    let addr = deploy_contract(
        &provider,
        from,
        creation_bytes(
            &output,
            "project/contracts/Scenarios.t.sol",
            "CustomErrorTest",
        )?,
    )?;

    let stack_trace =
        expect_failed_call_stack_trace(&provider, from, addr, call("testCustomError()"));

    assert!(
        stack_trace
            .iter()
            .any(|e| matches!(e, StackTraceEntry::CustomError { .. })),
        "expected a CustomError entry, got: {stack_trace:#?}"
    );
    Ok(())
}

const SCENARIOS_SOURCE: &str = "project/contracts/Scenarios.t.sol";

fn contains_ascii(data: &[u8], needle: &str) -> bool {
    data.windows(needle.len()).any(|w| w == needle.as_bytes())
}

/// Omits the embedded `source_content`, which makes `{:#?}` dumps of
/// [`SourceReference`] unreadable.
fn brief_trace(stack_trace: &[StackTraceEntry]) -> String {
    stack_trace
        .iter()
        .map(|entry| {
            let debug = format!("{entry:?}");
            let variant = debug.split_whitespace().next().unwrap_or("?").to_string();
            match entry.source_reference() {
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

#[track_caller]
fn assert_trace_shape(stack_trace: &[StackTraceEntry], expected: &[&str]) {
    assert_eq!(
        brief_trace(stack_trace),
        expected.join("\n"),
        "trace shape drifted — a gained frame may be a solx/inference \
         improvement (update the pin), a lost frame is a regression"
    );
}

#[track_caller]
fn assert_revert_at_line(stack_trace: &[StackTraceEntry], line: u32, reason: &str) {
    let entry = assert_single_variant(
        stack_trace,
        |e| matches!(e, StackTraceEntry::RevertError { .. }),
        "RevertError",
    );
    let StackTraceEntry::RevertError {
        return_data,
        source_reference,
        ..
    } = entry
    else {
        unreachable!("assert_single_variant matched a RevertError");
    };
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

// ---------- StackTraceScenarios fixture tests ----------

const STACK_TRACE_SCENARIOS_SOURCE: &str = "project/contracts/StackTraceScenarios.sol";

fn stack_trace_scenarios_provider(
) -> anyhow::Result<(Provider<L1ChainSpec>, Address, CompilerOutput<SolxBytecode>)> {
    let (build_info, output) = solx_stack_trace_scenarios_build_info(
        include_str!(
            "../../../edr_solidity/fixtures/solx_compiler_input_stack_trace_scenarios.json"
        ),
        include_str!(
            "../../../edr_solidity/fixtures/solx_compiler_output_stack_trace_scenarios.json"
        ),
    )?;
    let decoder = ContractDecoder::new(build_info);
    let (provider, from) = make_provider(decoder)?;
    Ok((provider, from, output))
}

/// Same scenarios compiled at optimizer mode 3: mode-1 DWARF is
/// statement-attributed since solx 0.1.6, so only these artifacts reach
/// the declaration-attributed and unmapped-revert inference paths.
fn stack_trace_scenarios_mode3_provider(
) -> anyhow::Result<(Provider<L1ChainSpec>, Address, CompilerOutput<SolxBytecode>)> {
    let (build_info, output) = solx_stack_trace_scenarios_build_info(
        include_str!(
            "../../../edr_solidity/fixtures/solx_compiler_input_stack_trace_scenarios_mode3.json"
        ),
        include_str!(
            "../../../edr_solidity/fixtures/solx_compiler_output_stack_trace_scenarios_mode3.json"
        ),
    )?;
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
    deploy_contract(
        provider,
        from,
        creation_bytes(output, STACK_TRACE_SCENARIOS_SOURCE, contract)?,
    )
}

#[track_caller]
fn assert_single_variant<'a>(
    stack_trace: &'a [StackTraceEntry],
    matcher: impl Fn(&StackTraceEntry) -> bool,
    variant: &str,
) -> &'a StackTraceEntry {
    let mut matches = stack_trace.iter().filter(|e| matcher(e));
    let entry = matches.next().unwrap_or_else(|| {
        panic!(
            "expected a {variant} entry, got:\n{}",
            brief_trace(stack_trace)
        )
    });
    assert!(
        matches.next().is_none(),
        "expected a single {variant} entry, got:\n{}",
        brief_trace(stack_trace)
    );
    entry
}

#[track_caller]
fn assert_returndata_size_error_at_call_get(stack_trace: &[StackTraceEntry]) {
    let entry = assert_single_variant(
        stack_trace,
        |e| matches!(e, StackTraceEntry::ReturndataSizeError { .. }),
        "ReturndataSizeError",
    );
    let source_reference = entry
        .source_reference()
        .expect("ReturndataSizeError carries a source reference");
    assert_eq!(source_reference.function.as_deref(), Some("callGet"));
    assert_eq!(
        source_reference.line,
        47,
        "expected the call-site line, got:\n{}",
        brief_trace(stack_trace)
    );
}

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

/// solc emits no EXTCODESIZE probe for returndata-expecting calls since
/// 0.8.10, so a returndata failure — not `NoncontractAccountCalledError` —
/// is the parity answer here.
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

#[tokio::test(flavor = "multi_thread")]
async fn nested_modifier_revert_points_at_the_failing_require() -> anyhow::Result<()> {
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
    assert_trace_shape(
        &stack_trace,
        &[
            "CallstackEntry project/contracts/StackTraceScenarios.sol:99 (ValidatedCounterCaller.callBump)",
            "CallstackEntry project/contracts/StackTraceScenarios.sol:86 (ValidatedCounter.bumpIfValid)",
            "RevertError project/contracts/StackTraceScenarios.sol:80 (ValidatedCounter.validates)",
        ],
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn bare_modifier_revert_attributes_to_the_revert_statement() -> anyhow::Result<()> {
    let (provider, from, output) = stack_trace_scenarios_provider()?;
    let addr = deploy_stack_trace_scenario(&provider, from, &output, "GuardedBareRevert")?;
    let stack_trace = expect_failed_call_stack_trace(&provider, from, addr, call("fire()"));
    let entry = assert_single_variant(
        &stack_trace,
        |e| matches!(e, StackTraceEntry::RevertError { .. }),
        "RevertError",
    );
    let source_reference = entry
        .source_reference()
        .expect("entry carries a source reference");
    assert_eq!(
        (source_reference.line, source_reference.function.as_deref()),
        (68, Some("guarded")),
        "expected the `revert()` statement line inside the modifier, got:\n{}",
        brief_trace(&stack_trace)
    );
    Ok(())
}

// ---------- mode-3 twins: pin the compat inference paths ----------

#[tokio::test(flavor = "multi_thread")]
async fn mode3_nested_modifier_revert_walks_back_to_the_failing_require() -> anyhow::Result<()> {
    let (provider, from, output) = stack_trace_scenarios_mode3_provider()?;
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

#[tokio::test(flavor = "multi_thread")]
async fn mode3_cross_contract_modifier_revert_keeps_called_function_frame() -> anyhow::Result<()> {
    let (provider, from, output) = stack_trace_scenarios_mode3_provider()?;
    let addr = deploy_stack_trace_scenario(&provider, from, &output, "ValidatedCounterCaller")?;
    let stack_trace = expect_failed_call_stack_trace(
        &provider,
        from,
        addr,
        encode_call_u256("callBump(uint256)", 13),
    );
    assert_revert_at_line(&stack_trace, 80, "unlucky");
    assert_trace_shape(
        &stack_trace,
        &[
            "CallstackEntry project/contracts/StackTraceScenarios.sol:99 (ValidatedCounterCaller.callBump)",
            "CallstackEntry project/contracts/StackTraceScenarios.sol:86 (ValidatedCounter.bumpIfValid)",
            "RevertError project/contracts/StackTraceScenarios.sol:80 (ValidatedCounter.validates)",
        ],
    );
    Ok(())
}

/// Line 67 is the `if (armed)` guard, not the `revert()` at 68: mode-3
/// DWARF gives the bare-revert entry path no statement line, so the
/// walk-back lands on the last statement that executed. Becomes 68 once
/// solx emits statement lines there.
#[tokio::test(flavor = "multi_thread")]
async fn mode3_bare_modifier_revert_recovers_the_failing_function() -> anyhow::Result<()> {
    let (provider, from, output) = stack_trace_scenarios_mode3_provider()?;
    let addr = deploy_stack_trace_scenario(&provider, from, &output, "GuardedBareRevert")?;
    let stack_trace = expect_failed_call_stack_trace(&provider, from, addr, call("fire()"));
    let entry = assert_single_variant(
        &stack_trace,
        |e| matches!(e, StackTraceEntry::RevertError { .. }),
        "RevertError",
    );
    let source_reference = entry
        .source_reference()
        .expect("entry carries a source reference");
    assert_eq!(
        (source_reference.line, source_reference.function.as_deref()),
        (67, Some("guarded")),
        "expected the guard line inside the modifier, got:\n{}",
        brief_trace(&stack_trace)
    );
    Ok(())
}
