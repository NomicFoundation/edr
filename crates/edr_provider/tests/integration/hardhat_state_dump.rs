//! Tests for `hardhat_dumpState` and `hardhat_loadState`
#![cfg(feature = "test-utils")]

use std::sync::Arc;

use anyhow::anyhow;
use edr_chain_l1::{Hardfork, L1ChainSpec};
use edr_primitives::{address, bytes, Address, Bytes, U256};
use edr_provider::{
    test_utils::{create_test_config_with, deploy_contract, MinimalProviderConfig},
    time::CurrentTime,
    MethodInvocation, NoopLogger, Provider, ProviderError, ProviderRequest,
};
use edr_signer::public_key_to_address;
use parking_lot::RwLock;
use tokio::runtime;

/// Stores 42 at slot 5 in its constructor and deploys the code `0x6001`.
const SLOT_5_CONTRACT: Bytes = bytes!("602a6005556160016000526002601ef3");

const ACCOUNT: Address = address!("0x00000000000000000000000000000000000000aa");

fn provider(
    provider_config: MinimalProviderConfig<Hardfork>,
) -> anyhow::Result<(Provider<L1ChainSpec, CurrentTime>, Option<Address>)> {
    let config = create_test_config_with(provider_config);
    let owner = config
        .owned_accounts
        .first()
        .map(|secret_key| public_key_to_address(secret_key.public_key()));
    let provider = Provider::new(
        runtime::Handle::current(),
        Box::new(NoopLogger::<L1ChainSpec>::default()),
        Box::new(|_event| {}),
        config,
        Arc::new(RwLock::default()),
        CurrentTime,
    )
    .map_err(|error| anyhow!(error))?;

    Ok((provider, owner))
}

fn request<ResultT: serde::de::DeserializeOwned>(
    provider: &Provider<L1ChainSpec, CurrentTime>,
    method: MethodInvocation<L1ChainSpec>,
) -> anyhow::Result<ResultT> {
    let response = provider.handle_request(ProviderRequest::with_single(method))?;
    Ok(response.deserialize_result()?)
}

fn dump_state(provider: &Provider<L1ChainSpec, CurrentTime>) -> anyhow::Result<Bytes> {
    request(provider, MethodInvocation::DumpState(()))
}

fn load_state(provider: &Provider<L1ChainSpec, CurrentTime>, dump: Bytes) -> anyhow::Result<bool> {
    request(provider, MethodInvocation::LoadState(dump))
}

fn balance(
    provider: &Provider<L1ChainSpec, CurrentTime>,
    address: Address,
) -> anyhow::Result<U256> {
    request(provider, MethodInvocation::GetBalance(address, None))
}

fn code(provider: &Provider<L1ChainSpec, CurrentTime>, address: Address) -> anyhow::Result<Bytes> {
    request(provider, MethodInvocation::GetCode(address, None))
}

fn nonce(provider: &Provider<L1ChainSpec, CurrentTime>, address: Address) -> anyhow::Result<U256> {
    request(
        provider,
        MethodInvocation::GetTransactionCount(address, None),
    )
}

fn storage(
    provider: &Provider<L1ChainSpec, CurrentTime>,
    address: Address,
    index: u64,
) -> anyhow::Result<U256> {
    let value: Bytes = request(
        provider,
        MethodInvocation::GetStorageAt(address, U256::from(index), None),
    )?;
    Ok(U256::from_be_slice(&value))
}

fn set(
    provider: &Provider<L1ChainSpec, CurrentTime>,
    method: MethodInvocation<L1ChainSpec>,
) -> anyhow::Result<()> {
    let result: bool = request(provider, method)?;
    assert!(result);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn restores_mined_and_irregular_state() -> anyhow::Result<()> {
    let (source, owner) = provider(MinimalProviderConfig::local_with_accounts())?;
    let owner = owner.expect("config should have an account");

    let contract = deploy_contract(&source, owner, SLOT_5_CONTRACT)?;
    set(
        &source,
        MethodInvocation::SetBalance(ACCOUNT, U256::from(5)),
    )?;
    set(&source, MethodInvocation::SetNonce(ACCOUNT, 3))?;
    set(&source, MethodInvocation::SetCode(ACCOUNT, bytes!("6002")))?;
    set(
        &source,
        MethodInvocation::SetStorageAt(ACCOUNT, U256::from(1), U256::from(7)),
    )?;

    let dump = dump_state(&source)?;

    let (target, _owner) = provider(MinimalProviderConfig::local_empty())?;
    assert!(load_state(&target, dump)?);

    assert_eq!(code(&target, contract)?, bytes!("6001"));
    assert_eq!(storage(&target, contract, 5)?, U256::from(42));
    assert_eq!(balance(&target, owner)?, balance(&source, owner)?);
    assert_eq!(nonce(&target, owner)?, U256::from(1));

    assert_eq!(balance(&target, ACCOUNT)?, U256::from(5));
    assert_eq!(nonce(&target, ACCOUNT)?, U256::from(3));
    assert_eq!(code(&target, ACCOUNT)?, bytes!("6002"));
    assert_eq!(storage(&target, ACCOUNT, 1)?, U256::from(7));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn dump_of_a_loaded_state_is_identical() -> anyhow::Result<()> {
    let (source, owner) = provider(MinimalProviderConfig::local_with_accounts())?;
    let owner = owner.expect("config should have an account");
    deploy_contract(&source, owner, SLOT_5_CONTRACT)?;
    set(
        &source,
        MethodInvocation::SetStorageAt(ACCOUNT, U256::from(1), U256::from(7)),
    )?;
    let dump = dump_state(&source)?;

    let (target, _owner) = provider(MinimalProviderConfig::local_empty())?;
    load_state(&target, dump.clone())?;

    assert_eq!(dump_state(&target)?, dump);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn load_merges_into_the_current_state() -> anyhow::Result<()> {
    let other = address!("0x00000000000000000000000000000000000000bb");

    let (source, _owner) = provider(MinimalProviderConfig::local_empty())?;
    set(
        &source,
        MethodInvocation::SetBalance(ACCOUNT, U256::from(5)),
    )?;
    set(&source, MethodInvocation::SetNonce(ACCOUNT, 2))?;
    set(
        &source,
        MethodInvocation::SetStorageAt(ACCOUNT, U256::from(1), U256::from(7)),
    )?;
    let dump = dump_state(&source)?;

    let (target, _owner) = provider(MinimalProviderConfig::local_empty())?;
    set(
        &target,
        MethodInvocation::SetBalance(ACCOUNT, U256::from(99)),
    )?;
    set(&target, MethodInvocation::SetNonce(ACCOUNT, 9))?;
    set(
        &target,
        MethodInvocation::SetStorageAt(ACCOUNT, U256::from(1), U256::from(1)),
    )?;
    set(
        &target,
        MethodInvocation::SetStorageAt(ACCOUNT, U256::from(2), U256::from(8)),
    )?;
    set(&target, MethodInvocation::SetBalance(other, U256::from(3)))?;

    load_state(&target, dump)?;

    // The dump's balance and storage win, the higher nonce is kept
    assert_eq!(balance(&target, ACCOUNT)?, U256::from(5));
    assert_eq!(nonce(&target, ACCOUNT)?, U256::from(9));
    assert_eq!(storage(&target, ACCOUNT, 1)?, U256::from(7));
    // Slots and accounts that aren't in the dump are kept
    assert_eq!(storage(&target, ACCOUNT, 2)?, U256::from(8));
    assert_eq!(balance(&target, other)?, U256::from(3));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn loaded_state_survives_mining() -> anyhow::Result<()> {
    let (source, _owner) = provider(MinimalProviderConfig::local_empty())?;
    set(
        &source,
        MethodInvocation::SetStorageAt(ACCOUNT, U256::from(1), U256::from(7)),
    )?;
    let dump = dump_state(&source)?;

    let (target, _owner) = provider(MinimalProviderConfig::local_empty())?;
    load_state(&target, dump)?;
    let _mined: serde_json::Value = request(&target, MethodInvocation::EvmMine(None))?;

    assert_eq!(storage(&target, ACCOUNT, 1)?, U256::from(7));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn state_rebuilt_from_history_includes_the_load() -> anyhow::Result<()> {
    let (source, _owner) = provider(MinimalProviderConfig::local_empty())?;
    set(
        &source,
        MethodInvocation::SetStorageAt(ACCOUNT, U256::from(1), U256::from(7)),
    )?;
    let dump = dump_state(&source)?;

    let (target, _owner) = provider(MinimalProviderConfig::local_empty())?;
    set(
        &target,
        MethodInvocation::SetStorageAt(ACCOUNT, U256::from(2), U256::from(8)),
    )?;
    let _mined: serde_json::Value = request(&target, MethodInvocation::EvmMine(None))?;
    load_state(&target, dump)?;
    let _mined: serde_json::Value = request(&target, MethodInvocation::EvmMine(None))?;

    // Reading block 1 rebuilds its state from the diffs, so the load has to be
    // recorded in them, not only in the cached latest state.
    let at_block_1 = |index: u64| -> anyhow::Result<U256> {
        let value: Bytes = request(
            &target,
            MethodInvocation::GetStorageAt(
                ACCOUNT,
                U256::from(index),
                Some(edr_eth::BlockSpec::Number(1)),
            ),
        )?;
        Ok(U256::from_be_slice(&value))
    };
    assert_eq!(at_block_1(1)?, U256::from(7));
    assert_eq!(at_block_1(2)?, U256::from(8));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn loads_plain_json_and_anvil_dumps() -> anyhow::Result<()> {
    let anvil_dump = r#"{
        "block": {"number": "0x2", "timestamp": "0x66cdc82b"},
        "accounts": {
            "0x00000000000000000000000000000000000000aa": {
                "nonce": 3,
                "balance": "0x5",
                "code": "0x6001",
                "storage": {"0x1": "0x2a"}
            }
        },
        "best_block_number": 2,
        "blocks": [],
        "transactions": [],
        "historical_states": null
    }"#;

    let (provider, _owner) = provider(MinimalProviderConfig::local_empty())?;
    load_state(&provider, Bytes::from(anvil_dump.as_bytes().to_vec()))?;

    assert_eq!(balance(&provider, ACCOUNT)?, U256::from(5));
    assert_eq!(nonce(&provider, ACCOUNT)?, U256::from(3));
    assert_eq!(code(&provider, ACCOUNT)?, bytes!("6001"));
    assert_eq!(storage(&provider, ACCOUNT, 1)?, U256::from(42));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_invalid_data() -> anyhow::Result<()> {
    let (provider, _owner) = provider(MinimalProviderConfig::local_empty())?;

    let error = provider
        .handle_request(ProviderRequest::with_single(MethodInvocation::LoadState(
            bytes!("deadbeef"),
        )))
        .expect_err("invalid data must be rejected");

    assert!(matches!(error, ProviderError::InvalidStateDump(_)));

    Ok(())
}
