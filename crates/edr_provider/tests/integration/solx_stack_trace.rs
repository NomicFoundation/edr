#![cfg(feature = "test-utils")]

//! Verifies the solx (DWARF) stack-trace path through the JSON-RPC
//! provider. Sections mirror the inference pipeline in `edr_solidity`, so
//! each test names the path it pins:
//!
//! - **Provider plumbing**: end-to-end smoke on the minimal Counter fixture.
//! - **Pre-execution guards** (`infer_before_tracing_call_message`):
//!   payability, missing function/fallback/receive, direct library calls.
//! - **Calldata decoding** (`check_last_instruction`): `InvalidParamsError`.
//! - **Revert/panic/custom attribution** (`check_revert_or_invalid_opcode`):
//!   panic codes with statement anchors, custom-error decoding, revert-line
//!   discrimination.
//! - **Callstack reconstruction** (frame push/pop, submessage splicing,
//!   `filter_redundant_frames`): cross-contract frames, recursion, internal
//!   helpers/libraries, linked external libraries, fallback/receive bodies.
//! - **Modifiers** (`fix_initial_modifier` + `SolxTraceStrategy` revert
//!   attribution): plain, nested, cross-contract, and bare-revert modifiers.
//! - **Create path** (creation-code `debugInfo`): reverting constructors.
//! - **Submessage checks** (`check_last_submessage`): returndata-size errors.
//! - **Mode-3 twins**: optimizer mode 3 artifacts reach the
//!   declaration-attributed and unmapped-revert strategy paths that mode-1
//!   DWARF (statement-attributed since solx 0.1.6) no longer hits.
//!
//! Deliberately not covered here: message-kind dispatch (precompiles,
//! unrecognized contracts, contract-too-large) and other location-free
//! classifications already pinned by the solc corpus, create-side guards,
//! the solc-opcode-pattern rewrites in
//! `mapped_inline_internal_functions_heuristics`, out-of-gas rewrites,
//! proxy propagation, and the `eth_call` channel — follow-up material.
//! Solidity-test runs are exercised by the JS parity sweep in
//! `js/integration-tests/solx-parity-sweep`.
//!
//! Line pins are goldens: after a solx upgrade a shifted anchor is expected
//! drift (update the pin), a lost frame is a regression.

use std::{collections::HashMap, sync::Arc};

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
    library_utils::link_hex_string_bytecode,
    solidity_stack_trace::{SourceReference, StackTraceCreationResult, StackTraceEntry},
};
use parking_lot::RwLock;
use tokio::runtime;

// ---------- fixture assembly ----------

const SCENARIOS_SOURCE: &str = "project/contracts/Scenarios.t.sol";
const STACK_TRACE_SCENARIOS_SOURCE: &str = "project/contracts/StackTraceScenarios.sol";

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

// ---------- provider setup and deployment ----------

/// Builds a local provider seeded with `decoder`, with bail-on-failure set
/// so a reverting tx surfaces as [`ProviderError::TransactionFailed`].
fn make_provider(decoder: ContractDecoder) -> anyhow::Result<(Provider<L1ChainSpec>, Address)> {
    let mut config = create_test_config_with(MinimalProviderConfig::local_with_accounts());
    config.bail_on_transaction_failure = true;

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

fn scenarios_provider(
) -> anyhow::Result<(Provider<L1ChainSpec>, Address, CompilerOutput<SolxBytecode>)> {
    let (build_info, output) = solx_scenarios_build_info()?;
    let decoder = ContractDecoder::new(build_info);
    let (provider, from) = make_provider(decoder)?;
    Ok((provider, from, output))
}

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

fn deploy_scenario(
    provider: &Provider<L1ChainSpec>,
    from: Address,
    output: &CompilerOutput<SolxBytecode>,
    contract: &str,
) -> anyhow::Result<Address> {
    deploy_contract(
        provider,
        from,
        creation_bytes(output, SCENARIOS_SOURCE, contract)?,
    )
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

/// Substitutes the deployed library address at every `linkReferences`
/// position, through the production `link_hex_string_bytecode` path.
fn link_library(bytecode: &SolxBytecode, library: Address) -> anyhow::Result<String> {
    let library_count: usize = bytecode.link_references.values().map(HashMap::len).sum();
    anyhow::ensure!(
        library_count == 1,
        "bytecode references {library_count} libraries; link_library takes one address"
    );
    let mut linked = bytecode.object.clone();
    for reference in bytecode
        .link_references
        .values()
        .flat_map(HashMap::values)
        .flatten()
    {
        linked = link_hex_string_bytecode(linked, &hex::encode(library), reference.start)?;
    }
    Ok(linked)
}

// ---------- calldata encoding ----------

fn selector(signature: &str) -> Selector {
    let hash = keccak256(signature.as_bytes());
    Selector::from(
        *hash
            .first_chunk::<4>()
            .expect("keccak256 output is 32 bytes"),
    )
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

// ---------- execution and stack-trace extraction ----------

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
    stack_trace_from_failure(err)
}

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

// ---------- assertions ----------

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

/// Asserts a single entry of `variant` and returns its source anchor.
#[track_caller]
fn assert_single_variant_anchor<'a>(
    stack_trace: &'a [StackTraceEntry],
    matcher: impl Fn(&StackTraceEntry) -> bool,
    variant: &str,
) -> &'a SourceReference {
    assert_single_variant(stack_trace, matcher, variant)
        .source_reference()
        .unwrap_or_else(|| {
            panic!(
                "expected the {variant} entry to carry a source reference, got:\n{}",
                brief_trace(stack_trace)
            )
        })
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

/// Pins both the decoded panic code and the statement the panic is
/// anchored to.
fn expect_scenario_panic(
    contract: &str,
    signature: &str,
    code: u64,
    line: u32,
) -> anyhow::Result<()> {
    let (provider, from, output) = scenarios_provider()?;
    let addr = deploy_scenario(&provider, from, &output, contract)?;
    let stack_trace = expect_failed_call_stack_trace(&provider, from, addr, call(signature));
    let entry = assert_single_variant(
        &stack_trace,
        |e| matches!(e, StackTraceEntry::PanicError { .. }),
        "PanicError",
    );
    let StackTraceEntry::PanicError {
        error_code,
        source_reference,
    } = entry
    else {
        unreachable!("assert_single_variant matched a PanicError");
    };
    assert_eq!(
        *error_code,
        U256::from(code),
        "expected panic code {code:#x}, got:\n{}",
        brief_trace(&stack_trace)
    );
    let source_reference = source_reference.as_ref().unwrap_or_else(|| {
        panic!(
            "expected the PanicError to carry a source reference, got:\n{}",
            brief_trace(&stack_trace)
        )
    });
    assert_eq!(
        source_reference.line,
        line,
        "expected the panic statement line, got:\n{}",
        brief_trace(&stack_trace)
    );
    Ok(())
}

// ---------- provider plumbing smoke ----------

/// Counter.set(0) reverts via `require(v > 0, "must be positive")`.
/// Pin: stack trace surfaces a [`StackTraceEntry::RevertError`] referencing
/// Counter.sol. Covers the provider-flow plumbing end-to-end on a second,
/// minimal artifact assembly (the Counter fixture).
#[tokio::test(flavor = "multi_thread")]
async fn revert_error_surfaces_end_to_end_for_counter() -> anyhow::Result<()> {
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

// ---------- pre-execution guards (infer_before_tracing_call_message) ------

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
    let anchor = assert_single_variant_anchor(
        &stack_trace,
        |e| matches!(e, StackTraceEntry::FunctionNotPayableError { .. }),
        "FunctionNotPayableError",
    );
    assert_eq!(
        (anchor.function.as_deref(), anchor.line),
        (Some("store"), 12),
        "expected the `store` declaration as anchor, got:\n{}",
        brief_trace(&stack_trace)
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn unrecognized_function_without_fallback_error_surfaces() -> anyhow::Result<()> {
    let (provider, from, output) = stack_trace_scenarios_provider()?;
    let addr = deploy_stack_trace_scenario(&provider, from, &output, "NoFallback")?;
    let stack_trace = expect_failed_call_stack_trace(&provider, from, addr, call("nonExistent()"));
    let anchor = assert_single_variant_anchor(
        &stack_trace,
        |e| {
            matches!(
                e,
                StackTraceEntry::UnrecognizedFunctionWithoutFallbackError { .. }
            )
        },
        "UnrecognizedFunctionWithoutFallbackError",
    );
    assert_eq!(
        (anchor.contract.as_deref(), anchor.line),
        (Some("NoFallback"), 17),
        "expected the contract declaration as anchor, got:\n{}",
        brief_trace(&stack_trace)
    );
    Ok(())
}

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
    let anchor = assert_single_variant_anchor(
        &stack_trace,
        |e| matches!(e, StackTraceEntry::MissingFallbackOrReceiveError { .. }),
        "MissingFallbackOrReceiveError",
    );
    assert_eq!(
        (anchor.contract.as_deref(), anchor.line),
        (Some("NoFallback"), 17),
        "expected the contract declaration as anchor, got:\n{}",
        brief_trace(&stack_trace)
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn fallback_not_payable_error_surfaces() -> anyhow::Result<()> {
    let (provider, from, output) = stack_trace_scenarios_provider()?;
    let addr = deploy_stack_trace_scenario(&provider, from, &output, "NonPayableFallback")?;
    let stack_trace = expect_failed_call_with_value_stack_trace(
        &provider,
        from,
        addr,
        call("nonExistent()"),
        U256::from(1u64),
    );
    let anchor = assert_single_variant_anchor(
        &stack_trace,
        |e| matches!(e, StackTraceEntry::FallbackNotPayableError { .. }),
        "FallbackNotPayableError",
    );
    assert_eq!(
        anchor.line,
        26,
        "expected the fallback declaration as anchor, got:\n{}",
        brief_trace(&stack_trace)
    );
    Ok(())
}

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
    let anchor = assert_single_variant_anchor(
        &stack_trace,
        |e| {
            matches!(
                e,
                StackTraceEntry::FallbackNotPayableAndNoReceiveError { .. }
            )
        },
        "FallbackNotPayableAndNoReceiveError",
    );
    assert_eq!(
        anchor.line,
        26,
        "expected the fallback declaration as anchor, got:\n{}",
        brief_trace(&stack_trace)
    );
    Ok(())
}

/// Calling a deployed library's external function directly.
#[tokio::test(flavor = "multi_thread")]
async fn direct_library_call_error_surfaces() -> anyhow::Result<()> {
    let (provider, from, output) = stack_trace_scenarios_provider()?;
    let library = deploy_stack_trace_scenario(&provider, from, &output, "ExternalLib")?;
    let stack_trace = expect_failed_call_stack_trace(&provider, from, library, call("fail()"));
    let anchor = assert_single_variant_anchor(
        &stack_trace,
        |e| matches!(e, StackTraceEntry::DirectLibraryCallError { .. }),
        "DirectLibraryCallError",
    );
    assert_eq!(
        (anchor.function.as_deref(), anchor.line),
        (Some("fail"), 52),
        "expected the called library function as anchor, got:\n{}",
        brief_trace(&stack_trace)
    );
    Ok(())
}

// ---------- calldata decoding (check_last_instruction) ----------

#[tokio::test(flavor = "multi_thread")]
async fn invalid_params_error_surfaces_for_truncated_calldata() -> anyhow::Result<()> {
    let (provider, from, output) = stack_trace_scenarios_provider()?;
    let addr = deploy_stack_trace_scenario(&provider, from, &output, "RequiresArgs")?;
    let mut calldata = selector("needsBoth(uint256,uint256)").as_slice().to_vec();
    calldata.extend_from_slice(&[0u8; 32]); // only one of the two words
    let stack_trace = expect_failed_call_stack_trace(&provider, from, addr, Bytes::from(calldata));
    let anchor = assert_single_variant_anchor(
        &stack_trace,
        |e| matches!(e, StackTraceEntry::InvalidParamsError { .. }),
        "InvalidParamsError",
    );
    assert_eq!(
        (anchor.function.as_deref(), anchor.line),
        (Some("needsBoth"), 32),
        "expected the called function declaration as anchor, got:\n{}",
        brief_trace(&stack_trace)
    );
    Ok(())
}

// ---------- revert/panic/custom (check_revert_or_invalid_opcode) ----------

#[tokio::test(flavor = "multi_thread")]
async fn panic_code_surfaces_for_assert_failure() -> anyhow::Result<()> {
    expect_scenario_panic("AssertionFailureTest", "testAssertionFails()", 0x01, 18)
}

#[tokio::test(flavor = "multi_thread")]
async fn panic_code_surfaces_for_arithmetic_overflow() -> anyhow::Result<()> {
    expect_scenario_panic("OverflowTest", "testOverflow()", 0x11, 26)
}

#[tokio::test(flavor = "multi_thread")]
async fn panic_code_surfaces_for_division_by_zero() -> anyhow::Result<()> {
    expect_scenario_panic("DivisionByZeroTest", "testDivisionByZero()", 0x12, 34)
}

#[tokio::test(flavor = "multi_thread")]
async fn panic_code_surfaces_for_invalid_enum_cast() -> anyhow::Result<()> {
    expect_scenario_panic("InvalidEnumCastTest", "testInvalidEnumCast()", 0x21, 169)
}

#[tokio::test(flavor = "multi_thread")]
async fn panic_code_surfaces_for_pop_on_empty_array() -> anyhow::Result<()> {
    expect_scenario_panic("PopEmptyArrayTest", "testPopEmpty()", 0x31, 177)
}

#[tokio::test(flavor = "multi_thread")]
async fn panic_code_surfaces_for_array_out_of_bounds() -> anyhow::Result<()> {
    expect_scenario_panic("ArrayOutOfBoundsTest", "testArrayOOB()", 0x32, 42)
}

/// `revert MyError(42, "custom error")`: pins the known-selector decode of
/// custom-error arguments into the message, not just the entry variant.
#[tokio::test(flavor = "multi_thread")]
async fn custom_error_decodes_name_and_args() -> anyhow::Result<()> {
    let (provider, from, output) = scenarios_provider()?;
    let addr = deploy_scenario(&provider, from, &output, "CustomErrorTest")?;
    let stack_trace =
        expect_failed_call_stack_trace(&provider, from, addr, call("testCustomError()"));
    let entry = assert_single_variant(
        &stack_trace,
        |e| matches!(e, StackTraceEntry::CustomError { .. }),
        "CustomError",
    );
    let StackTraceEntry::CustomError {
        message,
        source_reference,
    } = entry
    else {
        unreachable!("assert_single_variant matched a CustomError");
    };
    assert_eq!(
        message,
        r#"reverted with custom error 'MyError(42, "custom error")'"#,
        "got:\n{}",
        brief_trace(&stack_trace)
    );
    assert_eq!(
        source_reference.line,
        51,
        "expected the revert statement line, got:\n{}",
        brief_trace(&stack_trace)
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn revert_line_discriminates_between_requires() -> anyhow::Result<()> {
    expect_scenario_revert(
        "MultipleRequiresTest",
        call("testMultipleRequires()"),
        340,
        "second",
    )?;
    Ok(())
}

// ---------- callstack reconstruction (frames, submessages, recursion) ------

#[tokio::test(flavor = "multi_thread")]
async fn cross_contract_call_keeps_caller_frame() -> anyhow::Result<()> {
    let (provider, from, output) = scenarios_provider()?;
    let caller = deploy_scenario(&provider, from, &output, "CrossContractCallTest")?;
    send_ok(&provider, from, caller, call("setUp()"))?;
    let stack_trace =
        expect_failed_call_stack_trace(&provider, from, caller, call("testCrossContractCall()"));
    assert_revert_at_line(&stack_trace, 69, "called fail");
    // No separate `Other.fail` callstack frame: the bottom entry already
    // renders as `Other.fail`, so the intermediate frame dedups against it.
    assert_trace_shape(
        &stack_trace,
        &[
            "CallstackEntry project/contracts/Scenarios.t.sol:81 (CrossContractCallTest.testCrossContractCall)",
            "RevertError project/contracts/Scenarios.t.sol:69 (Other.fail)",
        ],
    );
    Ok(())
}

/// One frame per external call, not collapsed by `filter_redundant_frames`
/// (solx `recursion_start_idx` = 0).
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
    assert_trace_shape(
        &stack_trace,
        &[
            "CallstackEntry project/contracts/Scenarios.t.sol:111 (DeepRecursionTarget.recurse)",
            "CallstackEntry project/contracts/Scenarios.t.sol:111 (DeepRecursionTarget.recurse)",
            "CallstackEntry project/contracts/Scenarios.t.sol:111 (DeepRecursionTarget.recurse)",
            "RevertError project/contracts/Scenarios.t.sol:109 (DeepRecursionTarget.recurse)",
        ],
    );
    Ok(())
}

/// solx's optimizer may unroll the recursion, so only the bottom revert
/// line is pinned.
#[tokio::test(flavor = "multi_thread")]
async fn internal_recursion_pins_bottom_revert_line() -> anyhow::Result<()> {
    expect_scenario_revert(
        "InternalRecurseTest",
        call("testInternalRecurse()"),
        348,
        "internal bottom",
    )?;
    Ok(())
}

/// solx may emit both JUMP-derived and inlined frames at the dispatch
/// points, so only the bottom revert line is pinned.
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

#[tokio::test(flavor = "multi_thread")]
async fn internal_library_revert_points_into_library() -> anyhow::Result<()> {
    expect_scenario_revert(
        "LibraryRevertTest",
        call("testLibraryRevert()"),
        229,
        "lib boom",
    )?;
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
        .bytecode;
    let linked = link_library(unlinked, library)?;
    let user = deploy_contract(&provider, from, Bytes::from(hex::decode(&linked)?))?;

    let stack_trace = expect_failed_call_stack_trace(&provider, from, user, call("go()"));
    assert_revert_at_line(&stack_trace, 53, "external lib boom");
    assert_trace_shape(
        &stack_trace,
        &[
            "CallstackEntry project/contracts/StackTraceScenarios.sol:59 (UsesExternalLib.go)",
            "RevertError project/contracts/StackTraceScenarios.sol:53 (ExternalLib.fail)",
        ],
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn fallback_revert_points_at_fallback_body() -> anyhow::Result<()> {
    expect_scenario_revert(
        "FallbackRevertTarget",
        call("nonExistent()"),
        259,
        "fallback boom",
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn receive_revert_points_at_receive_body() -> anyhow::Result<()> {
    expect_scenario_revert("ReceiveRevertTarget", Bytes::new(), 276, "receive boom")?;
    Ok(())
}

// ---------- modifiers (fix_initial_modifier + strategy attribution) ----------

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
    let anchor = assert_single_variant_anchor(
        &stack_trace,
        |e| matches!(e, StackTraceEntry::RevertError { .. }),
        "RevertError",
    );
    assert_eq!(
        (anchor.line, anchor.function.as_deref()),
        (68, Some("guarded")),
        "expected the `revert()` statement line inside the modifier, got:\n{}",
        brief_trace(&stack_trace)
    );
    Ok(())
}

// ---------- create path (creation-code debugInfo) ----------

/// Resolves through `evm.bytecode.debugInfo` (creation code) rather than
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

// ---------- submessage checks (check_last_submessage) ----------

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
    let anchor = assert_single_variant_anchor(
        &stack_trace,
        |e| matches!(e, StackTraceEntry::RevertError { .. }),
        "RevertError",
    );
    assert_eq!(
        (anchor.line, anchor.function.as_deref()),
        (67, Some("guarded")),
        "expected the guard line inside the modifier, got:\n{}",
        brief_trace(&stack_trace)
    );
    Ok(())
}
