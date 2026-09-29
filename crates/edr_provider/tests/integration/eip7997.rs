#![cfg(feature = "test-utils")]

//! [EIP-7997]: Deterministic Factory Contract.
//!
//! The factory performs a `CREATE2` with the first 32 bytes of calldata as
//! salt, the remainder as init code and the call value forwarded. On success
//! it returns the created address as exactly 20 bytes. If creation fails it
//! reverts with empty data; calldata shorter than 32 bytes runs out of gas.
//!
//! The provider takes its genesis state from the caller, so these tests seed
//! the factory themselves and exercise the contract's behaviour rather than
//! the hardfork gating, which lives in the N-API `l1GenesisState`.
//!
//! [EIP-7997]: https://eips.ethereum.org/EIPS/eip-7997

use edr_chain_l1::{
    rpc::{call::L1CallRequest, TransactionRequest},
    L1ChainSpec,
};
use edr_eip7997::{DETERMINISTIC_FACTORY_ADDRESS, DETERMINISTIC_FACTORY_BYTECODE};
use edr_primitives::{address, b256, Address, Bytecode, Bytes, B256};
use edr_provider::{
    config::AccountOverride, MethodInvocation, Provider, ProviderError, ProviderErrorForChainSpec,
    ProviderRequest, TransactionFailureReason,
};

use crate::common::{
    bytecode::BytecodeBuilder,
    provider::{new_provider_with_config, send_transaction},
};

const SENDER: Address = address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

const SALT: B256 = b256!("0x0000000000000000000000000000000000000000000000000000000000000001");

/// Value the deployed contract returns, to tell it apart from an empty
/// account.
const MARKER: u8 = 42;

fn provider_with_factory() -> anyhow::Result<Provider<L1ChainSpec>> {
    new_provider_with_config(|config| {
        config.hardfork = edr_chain_l1::Hardfork::Amsterdam;
        // Surface failed calls as errors instead of empty output.
        config.bail_on_call_failure = true;
        config.genesis_state.insert(
            DETERMINISTIC_FACTORY_ADDRESS,
            AccountOverride {
                nonce: Some(1),
                code: Some(Bytecode::new_raw(DETERMINISTIC_FACTORY_BYTECODE)),
                ..AccountOverride::default()
            },
        );
    })
}

fn runtime_code() -> Bytes {
    let mut builder = BytecodeBuilder::default();
    builder.push1(MARKER).return_top_word();
    builder.runtime()
}

fn init_code() -> Bytes {
    let mut builder = BytecodeBuilder::default();
    builder.push1(MARKER).return_top_word();
    builder.deployable()
}

/// Factory calldata: `salt ++ init_code`.
fn factory_calldata(salt: B256, init_code: &Bytes) -> Bytes {
    [salt.as_slice(), init_code].concat().into()
}

fn call_factory(provider: &Provider<L1ChainSpec>, data: Bytes) -> anyhow::Result<Bytes> {
    let request = L1CallRequest {
        from: Some(SENDER),
        to: Some(DETERMINISTIC_FACTORY_ADDRESS),
        data: Some(data),
        ..L1CallRequest::default()
    };

    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::Call(request, None, None),
    ))?;

    Ok(response.deserialize_result()?)
}

/// Asserts that the factory call failed with a reason accepted by
/// `is_expected`.
fn assert_failed_with(
    result: anyhow::Result<Bytes>,
    context: &str,
    is_expected: impl Fn(&TransactionFailureReason<edr_chain_l1::HaltReason>) -> bool,
) {
    let error = match result {
        Ok(returned) => panic!("{context} should fail, got {returned}"),
        Err(error) => error,
    };

    assert!(
        matches!(
            error.downcast_ref::<ProviderErrorForChainSpec<L1ChainSpec>>(),
            Some(ProviderError::TransactionFailed(failure)) if is_expected(&failure.failure.reason)
        ),
        "{context} failed with an unexpected reason: {error:?}"
    );
}

/// Deploys `init_code` through the factory with `salt` and asserts the runtime
/// code landed at the CREATE2 address, which it returns.
fn deploy_through_factory(
    provider: &Provider<L1ChainSpec>,
    salt: B256,
    init_code: &Bytes,
) -> anyhow::Result<Address> {
    send_transaction(
        provider,
        TransactionRequest {
            from: SENDER,
            to: Some(DETERMINISTIC_FACTORY_ADDRESS),
            data: Some(factory_calldata(salt, init_code)),
            ..TransactionRequest::default()
        },
    )?;

    let deployed = DETERMINISTIC_FACTORY_ADDRESS.create2_from_code(salt, init_code);
    assert_eq!(
        code_at(provider, deployed)?,
        runtime_code(),
        "the runtime code should live at the CREATE2 address"
    );

    Ok(deployed)
}

fn code_at(provider: &Provider<L1ChainSpec>, address: Address) -> anyhow::Result<Bytes> {
    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetCode(address, None),
    ))?;

    Ok(response.deserialize_result()?)
}

#[tokio::test(flavor = "multi_thread")]
async fn factory_returns_unpadded_create2_address() -> anyhow::Result<()> {
    let provider = provider_with_factory()?;
    let init_code = init_code();

    let returned = call_factory(&provider, factory_calldata(SALT, &init_code))?;

    let expected = DETERMINISTIC_FACTORY_ADDRESS.create2_from_code(SALT, &init_code);
    assert_eq!(
        returned.as_ref(),
        expected.as_slice(),
        "factory should return the CREATE2 address as exactly 20 bytes"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn factory_deploys_at_create2_address() -> anyhow::Result<()> {
    let provider = provider_with_factory()?;

    deploy_through_factory(&provider, SALT, &init_code())?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn factory_runs_out_of_gas_on_calldata_shorter_than_salt() -> anyhow::Result<()> {
    let provider = provider_with_factory()?;

    let result = call_factory(&provider, Bytes::from_static(&[0u8; 31]));

    assert_failed_with(result, "31 bytes of calldata", |reason| {
        matches!(reason, TransactionFailureReason::OutOfGas(_))
    });

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn factory_reverts_on_reused_salt() -> anyhow::Result<()> {
    let provider = provider_with_factory()?;
    let init_code = init_code();
    deploy_through_factory(&provider, SALT, &init_code)?;

    let result = call_factory(&provider, factory_calldata(SALT, &init_code));

    assert_failed_with(
        result,
        "a second deployment with the same salt",
        |reason| matches!(reason, TransactionFailureReason::Revert(data) if data.is_empty()),
    );

    Ok(())
}
