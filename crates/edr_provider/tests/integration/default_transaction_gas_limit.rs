#![cfg(feature = "test-utils")]

//! Without a configured default, requests without a `gas` value derive their
//! gas limit from the block they execute in.

use std::num::NonZeroU64;

use edr_chain_l1::{
    rpc::{call::L1CallRequest, TransactionRequest},
    L1ChainSpec,
};
use edr_eip7825::OSAKA_TRANSACTION_GAS_CAP;
use edr_eth::BlockSpec;
use edr_primitives::{address, bytes, Address, Bytes, U256, U64};
use edr_provider::{
    config::{ConfigOption, NetworkConfig, ProviderConfig},
    MethodInvocation, Provider, ProviderRequest,
};

use crate::common::provider::{new_provider_with_config, send_transaction};

const CALLER: Address = address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
const GAS_REPORTER_ADDRESS: Address = address!("0x1000000000000000000000000000000000000001");
/// Returns the gas left after executing `GAS` as a 32-byte word.
const GAS_REPORTER_CODE: Bytes = bytes!("5a60005260206000f3");
const TX_BASE_COST: u64 = 21_000;
const GAS_OPCODE_COST: u64 = 2;
/// The gas left that `GAS` reports excludes its own cost.
const GAS_SPENT_BEFORE_REPORT: u64 = TX_BASE_COST + GAS_OPCODE_COST;
/// Below `OSAKA_TRANSACTION_GAS_CAP`.
const SMALL_BLOCK_GAS_LIMIT: u64 = 10_000_000;
/// Above `OSAKA_TRANSACTION_GAS_CAP`.
const LARGE_BLOCK_GAS_LIMIT: u64 = 30_000_000;
/// Below `OSAKA_TRANSACTION_GAS_CAP`.
const PENDING_BLOCK_GAS_LIMIT: u64 = 8_000_000;
/// Below the block gas limit.
const CUSTOM_TRANSACTION_GAS_CAP: u64 = 1_000_000;
/// Distinct from every derived limit.
const CONFIGURED_DEFAULT_GAS_LIMIT: u64 = 5_000_000;

fn use_hardfork_without_default_gas_limit(
    config: &mut ProviderConfig<edr_chain_l1::Hardfork>,
    hardfork: edr_chain_l1::Hardfork,
    transaction_gas_cap: ConfigOption<u64>,
) {
    config.hardfork = hardfork;
    config.default_transaction_gas_limit = None;
    config.transaction_gas_cap = transaction_gas_cap;
}

fn non_zero(gas_limit: u64) -> NonZeroU64 {
    NonZeroU64::new(gas_limit).expect("gas limit should be non-zero")
}

fn set_genesis_block_gas_limit(
    config: &mut ProviderConfig<edr_chain_l1::Hardfork>,
    block_gas_limit: u64,
) {
    let NetworkConfig::Local(local_config) = &mut config.network else {
        panic!("genesis block gas limit requires a local network");
    };
    local_config.genesis_block_gas_limit = non_zero(block_gas_limit);
}

fn set_block_gas_limit(config: &mut ProviderConfig<edr_chain_l1::Hardfork>, block_gas_limit: u64) {
    config.mining.block_gas_limit = Some(non_zero(block_gas_limit));
    set_genesis_block_gas_limit(config, block_gas_limit);
}

fn deploy_gas_reporter(provider: &Provider<L1ChainSpec>) -> anyhow::Result<()> {
    provider.handle_request(ProviderRequest::with_single(MethodInvocation::SetCode(
        GAS_REPORTER_ADDRESS,
        GAS_REPORTER_CODE,
    )))?;

    Ok(())
}

/// Creates a provider without a default transaction gas limit and deploys
/// the gas reporter. `customize` adjusts the config further.
fn new_gas_reporter_provider(
    hardfork: edr_chain_l1::Hardfork,
    transaction_gas_cap: ConfigOption<u64>,
    customize: impl FnOnce(&mut ProviderConfig<edr_chain_l1::Hardfork>),
) -> anyhow::Result<Provider<L1ChainSpec>> {
    let provider = new_provider_with_config(|config| {
        use_hardfork_without_default_gas_limit(config, hardfork, transaction_gas_cap);
        customize(config);
    })?;
    deploy_gas_reporter(&provider)?;

    Ok(provider)
}

/// Parses the quantity `field` of a JSON-RPC object.
fn quantity_field(object: &serde_json::Value, field: &str) -> anyhow::Result<u64> {
    let quantity = object
        .get(field)
        .ok_or_else(|| anyhow::anyhow!("object should have a `{field}` field"))?;
    let quantity: U64 = serde_json::from_value(quantity.clone())?;

    Ok(quantity.to())
}

/// Calls the gas reporter without a `gas` value and returns its gas limit.
fn call_without_gas(
    provider: &Provider<L1ChainSpec>,
    block_spec: Option<BlockSpec>,
) -> anyhow::Result<u64> {
    let request = L1CallRequest {
        from: Some(CALLER),
        to: Some(GAS_REPORTER_ADDRESS),
        ..L1CallRequest::default()
    };

    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::Call(request, block_spec, None),
    ))?;

    let output: Bytes = response.deserialize_result()?;
    let gas_left: u64 = U256::try_from_be_slice(&output)
        .ok_or_else(|| anyhow::anyhow!("output should fit in a 32-byte word"))?
        .to();

    Ok(gas_left + GAS_SPENT_BEFORE_REPORT)
}

/// Sends a transaction without a `gas` value and returns its gas limit.
fn send_transaction_without_gas(provider: &Provider<L1ChainSpec>) -> anyhow::Result<u64> {
    let transaction_hash = send_transaction(
        provider,
        TransactionRequest {
            from: CALLER,
            to: Some(GAS_REPORTER_ADDRESS),
            ..TransactionRequest::default()
        },
    )?;

    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetTransactionByHash(transaction_hash),
    ))?;

    let transaction: Option<serde_json::Value> = response.deserialize_result()?;
    let transaction = transaction.ok_or_else(|| anyhow::anyhow!("transaction should exist"))?;

    quantity_field(&transaction, "gas")
}

#[tokio::test(flavor = "multi_thread")]
async fn call_and_transaction_without_gas_use_osaka_cap() -> anyhow::Result<()> {
    let provider = new_gas_reporter_provider(
        edr_chain_l1::Hardfork::Osaka,
        ConfigOption::Default,
        |_config| {},
    )?;

    assert_eq!(
        call_without_gas(&provider, None)?,
        OSAKA_TRANSACTION_GAS_CAP
    );
    assert_eq!(
        send_transaction_without_gas(&provider)?,
        OSAKA_TRANSACTION_GAS_CAP
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn call_without_gas_is_capped_by_block_gas_limit() -> anyhow::Result<()> {
    let provider = new_gas_reporter_provider(
        edr_chain_l1::Hardfork::Osaka,
        ConfigOption::Default,
        |config| {
            set_block_gas_limit(config, SMALL_BLOCK_GAS_LIMIT);
        },
    )?;

    let gas_limit = call_without_gas(&provider, None)?;

    assert_eq!(gas_limit, SMALL_BLOCK_GAS_LIMIT);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn call_and_transaction_without_gas_use_custom_transaction_gas_cap() -> anyhow::Result<()> {
    let provider = new_gas_reporter_provider(
        edr_chain_l1::Hardfork::Osaka,
        ConfigOption::Custom(CUSTOM_TRANSACTION_GAS_CAP),
        |_config| {},
    )?;

    assert_eq!(
        call_without_gas(&provider, None)?,
        CUSTOM_TRANSACTION_GAS_CAP
    );
    assert_eq!(
        send_transaction_without_gas(&provider)?,
        CUSTOM_TRANSACTION_GAS_CAP
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn call_without_gas_uses_block_gas_limit_when_transaction_gas_cap_is_disabled(
) -> anyhow::Result<()> {
    let provider = new_gas_reporter_provider(
        edr_chain_l1::Hardfork::Osaka,
        ConfigOption::Disable,
        |config| set_block_gas_limit(config, LARGE_BLOCK_GAS_LIMIT),
    )?;

    assert_eq!(call_without_gas(&provider, None)?, LARGE_BLOCK_GAS_LIMIT);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn transaction_without_gas_uses_block_gas_limit_from_amsterdam() -> anyhow::Result<()> {
    let provider = new_gas_reporter_provider(
        edr_chain_l1::Hardfork::Amsterdam,
        ConfigOption::Default,
        |config| set_block_gas_limit(config, LARGE_BLOCK_GAS_LIMIT),
    )?;

    assert_eq!(
        send_transaction_without_gas(&provider)?,
        LARGE_BLOCK_GAS_LIMIT
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn transaction_without_gas_uses_parent_gas_limit_without_mining_gas_limit(
) -> anyhow::Result<()> {
    let provider = new_gas_reporter_provider(
        edr_chain_l1::Hardfork::Osaka,
        ConfigOption::Default,
        |config| {
            config.mining.block_gas_limit = None;
            set_genesis_block_gas_limit(config, SMALL_BLOCK_GAS_LIMIT);
        },
    )?;

    assert_eq!(
        send_transaction_without_gas(&provider)?,
        SMALL_BLOCK_GAS_LIMIT
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn pending_call_and_transaction_without_gas_use_mining_gas_limit() -> anyhow::Result<()> {
    let provider = new_gas_reporter_provider(
        edr_chain_l1::Hardfork::Osaka,
        ConfigOption::Default,
        |config| {
            config.mining.block_gas_limit = Some(non_zero(PENDING_BLOCK_GAS_LIMIT));
            set_genesis_block_gas_limit(config, LARGE_BLOCK_GAS_LIMIT);
        },
    )?;

    assert_eq!(
        call_without_gas(&provider, Some(BlockSpec::pending()))?,
        PENDING_BLOCK_GAS_LIMIT
    );
    assert_eq!(
        call_without_gas(&provider, Some(BlockSpec::latest()))?,
        OSAKA_TRANSACTION_GAS_CAP
    );
    assert_eq!(
        send_transaction_without_gas(&provider)?,
        PENDING_BLOCK_GAS_LIMIT
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn configured_default_gas_limit_takes_precedence() -> anyhow::Result<()> {
    let provider = new_gas_reporter_provider(
        edr_chain_l1::Hardfork::Osaka,
        ConfigOption::Default,
        |config| {
            config.default_transaction_gas_limit = Some(non_zero(CONFIGURED_DEFAULT_GAS_LIMIT));
        },
    )?;

    assert_eq!(
        call_without_gas(&provider, None)?,
        CONFIGURED_DEFAULT_GAS_LIMIT
    );
    assert_eq!(
        send_transaction_without_gas(&provider)?,
        CONFIGURED_DEFAULT_GAS_LIMIT
    );

    Ok(())
}

#[cfg(feature = "test-remote")]
mod fork {
    use edr_primitives::HashMap;
    use edr_provider::{
        config::ForkConfig,
        test_utils::{
            create_test_config_with, get_latest_block, mine_block, MinimalProviderConfig,
        },
    };
    use edr_test_utils::env::json_rpc_url_provider;

    use super::*;
    use crate::common::provider::new_provider_from_config;

    /// A mainnet block before the Osaka activation, with a gas limit above
    /// `OSAKA_TRANSACTION_GAS_CAP`.
    const PRAGUE_FORK_BLOCK_NUMBER: u64 = 23_000_000;
    /// A mainnet block after the Osaka activation.
    const OSAKA_FORK_BLOCK_NUMBER: u64 = 24_000_000;

    /// Creates a mainnet fork provider without a default transaction gas
    /// limit and deploys the gas reporter.
    fn new_fork_gas_reporter_provider(
        fork_block_number: u64,
        hardfork: edr_chain_l1::Hardfork,
        transaction_gas_cap: ConfigOption<u64>,
    ) -> anyhow::Result<Provider<L1ChainSpec>> {
        let mut config =
            create_test_config_with(MinimalProviderConfig::fork_with_accounts(ForkConfig {
                block_number: Some(fork_block_number),
                cache_dir: edr_defaults::CACHE_DIR.into(),
                chain_overrides: HashMap::default(),
                http_headers: None,
                url: json_rpc_url_provider::ethereum_mainnet(),
            }));
        use_hardfork_without_default_gas_limit(&mut config, hardfork, transaction_gas_cap);

        let provider = new_provider_from_config(config)?;
        deploy_gas_reporter(&provider)?;

        Ok(provider)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn call_without_gas_at_prague_block_is_capped_by_enforced_osaka_cap() -> anyhow::Result<()>
    {
        let provider = new_fork_gas_reporter_provider(
            PRAGUE_FORK_BLOCK_NUMBER,
            edr_chain_l1::Hardfork::Osaka,
            ConfigOption::Default,
        )?;

        let gas_limit =
            call_without_gas(&provider, Some(BlockSpec::Number(PRAGUE_FORK_BLOCK_NUMBER)))?;
        assert_eq!(gas_limit, OSAKA_TRANSACTION_GAS_CAP);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn call_without_gas_follows_the_executing_block_hardfork() -> anyhow::Result<()> {
        let provider = new_fork_gas_reporter_provider(
            OSAKA_FORK_BLOCK_NUMBER,
            edr_chain_l1::Hardfork::Amsterdam,
            ConfigOption::Default,
        )?;

        let gas_limit =
            call_without_gas(&provider, Some(BlockSpec::Number(OSAKA_FORK_BLOCK_NUMBER)))?;
        assert_eq!(gas_limit, OSAKA_TRANSACTION_GAS_CAP);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn call_without_gas_at_osaka_block_uses_osaka_cap_on_prague_provider(
    ) -> anyhow::Result<()> {
        let provider = new_fork_gas_reporter_provider(
            OSAKA_FORK_BLOCK_NUMBER,
            edr_chain_l1::Hardfork::Prague,
            ConfigOption::Default,
        )?;

        let gas_limit =
            call_without_gas(&provider, Some(BlockSpec::Number(OSAKA_FORK_BLOCK_NUMBER)))?;
        assert_eq!(gas_limit, OSAKA_TRANSACTION_GAS_CAP);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn call_and_transaction_without_gas_use_mined_amsterdam_block_gas_limit_with_disabled_cap(
    ) -> anyhow::Result<()> {
        let provider = new_fork_gas_reporter_provider(
            OSAKA_FORK_BLOCK_NUMBER,
            edr_chain_l1::Hardfork::Amsterdam,
            // The gas reporter observes execution gas, so it must span `tx.gas`.
            ConfigOption::Disable,
        )?;

        mine_block(&provider);
        let block_gas_limit = quantity_field(&get_latest_block(&provider), "gasLimit")?;

        let gas_limit = call_without_gas(&provider, None)?;
        assert_eq!(gas_limit, block_gas_limit);

        let gas_limit = send_transaction_without_gas(&provider)?;
        assert_eq!(gas_limit, block_gas_limit);

        Ok(())
    }
}
