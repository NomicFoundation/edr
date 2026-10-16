//! Without a configured default, requests without a `gas` value derive their
//! gas limit from the block they execute in.
#![cfg(feature = "test-utils")]

use std::num::NonZeroU64;

use edr_chain_l1::L1ChainSpec;
use edr_eip7825::OSAKA_TRANSACTION_GAS_CAP;
use edr_eth::BlockSpec;
use edr_provider::{
    config::{ConfigOption, NetworkConfig, ProviderConfig},
    Provider,
};

use crate::common::{
    gas_reporter::{call_without_gas, deploy_gas_reporter, send_transaction_without_gas},
    provider::new_provider_with_config,
};

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

#[tokio::test(flavor = "multi_thread")]
async fn call_and_transaction_without_gas_use_osaka_cap() -> anyhow::Result<()> {
    let provider = new_gas_reporter_provider(
        edr_chain_l1::Hardfork::Osaka,
        ConfigOption::Default,
        |_config| {},
    )?;

    assert_eq!(
        call_without_gas(&provider, None, edr_chain_l1::Hardfork::Osaka)?,
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

    let gas_limit = call_without_gas(&provider, None, edr_chain_l1::Hardfork::Osaka)?;

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
        call_without_gas(&provider, None, edr_chain_l1::Hardfork::Osaka)?,
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

    assert_eq!(
        call_without_gas(&provider, None, edr_chain_l1::Hardfork::Osaka)?,
        LARGE_BLOCK_GAS_LIMIT
    );

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
        call_without_gas(
            &provider,
            Some(BlockSpec::pending()),
            edr_chain_l1::Hardfork::Osaka
        )?,
        PENDING_BLOCK_GAS_LIMIT
    );
    assert_eq!(
        call_without_gas(
            &provider,
            Some(BlockSpec::latest()),
            edr_chain_l1::Hardfork::Osaka
        )?,
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
        call_without_gas(&provider, None, edr_chain_l1::Hardfork::Osaka)?,
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
    use edr_eth::PreEip1898BlockSpec;
    use edr_provider::{test_utils::mine_block, MethodInvocation, ProviderRequest};

    use super::*;
    use crate::common::provider::{
        mainnet_fork_config, new_provider_from_config, quantity_field, OSAKA_MAINNET_BLOCK_NUMBER,
        PRAGUE_MAINNET_BLOCK_NUMBER,
    };

    /// Creates a mainnet fork provider without a default transaction gas
    /// limit and deploys the gas reporter.
    fn new_fork_gas_reporter_provider(
        fork_block_number: u64,
        hardfork: edr_chain_l1::Hardfork,
        transaction_gas_cap: ConfigOption<u64>,
    ) -> anyhow::Result<Provider<L1ChainSpec>> {
        let mut config = mainnet_fork_config(fork_block_number);
        use_hardfork_without_default_gas_limit(&mut config, hardfork, transaction_gas_cap);

        let provider = new_provider_from_config(config)?;
        deploy_gas_reporter(&provider)?;

        Ok(provider)
    }

    /// Returns the gas limit of the block at `block_spec`.
    fn block_gas_limit(
        provider: &Provider<L1ChainSpec>,
        block_spec: PreEip1898BlockSpec,
    ) -> anyhow::Result<u64> {
        let response = provider.handle_request(ProviderRequest::with_single(
            MethodInvocation::GetBlockByNumber(block_spec, false),
        ))?;

        let block: Option<serde_json::Value> = response.deserialize_result()?;
        let block = block.ok_or_else(|| anyhow::anyhow!("block should exist"))?;

        quantity_field(&block, "gasLimit")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn call_without_gas_at_prague_block_uses_block_gas_limit_on_osaka_provider(
    ) -> anyhow::Result<()> {
        let provider = new_fork_gas_reporter_provider(
            PRAGUE_MAINNET_BLOCK_NUMBER,
            edr_chain_l1::Hardfork::Osaka,
            ConfigOption::Default,
        )?;

        let gas_limit = call_without_gas(
            &provider,
            Some(BlockSpec::Number(PRAGUE_MAINNET_BLOCK_NUMBER)),
            edr_chain_l1::Hardfork::Prague,
        )?;
        assert_eq!(
            gas_limit,
            block_gas_limit(
                &provider,
                PreEip1898BlockSpec::Number(PRAGUE_MAINNET_BLOCK_NUMBER)
            )?
        );

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn call_without_gas_follows_the_executing_block_hardfork() -> anyhow::Result<()> {
        let provider = new_fork_gas_reporter_provider(
            OSAKA_MAINNET_BLOCK_NUMBER,
            edr_chain_l1::Hardfork::Amsterdam,
            ConfigOption::Default,
        )?;

        let gas_limit = call_without_gas(
            &provider,
            Some(BlockSpec::Number(OSAKA_MAINNET_BLOCK_NUMBER)),
            edr_chain_l1::Hardfork::Osaka,
        )?;
        assert_eq!(gas_limit, OSAKA_TRANSACTION_GAS_CAP);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn call_without_gas_at_osaka_block_uses_osaka_cap_on_prague_provider(
    ) -> anyhow::Result<()> {
        let provider = new_fork_gas_reporter_provider(
            OSAKA_MAINNET_BLOCK_NUMBER,
            edr_chain_l1::Hardfork::Prague,
            ConfigOption::Default,
        )?;

        let gas_limit = call_without_gas(
            &provider,
            Some(BlockSpec::Number(OSAKA_MAINNET_BLOCK_NUMBER)),
            edr_chain_l1::Hardfork::Osaka,
        )?;
        assert_eq!(gas_limit, OSAKA_TRANSACTION_GAS_CAP);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn call_and_transaction_without_gas_use_mined_amsterdam_block_gas_limit_with_disabled_cap(
    ) -> anyhow::Result<()> {
        let provider = new_fork_gas_reporter_provider(
            OSAKA_MAINNET_BLOCK_NUMBER,
            edr_chain_l1::Hardfork::Amsterdam,
            // The gas reporter observes execution gas, so it must span `tx.gas`.
            ConfigOption::Disable,
        )?;

        mine_block(&provider);
        let mined_block_gas_limit = block_gas_limit(&provider, PreEip1898BlockSpec::latest())?;

        let gas_limit = call_without_gas(&provider, None, edr_chain_l1::Hardfork::Amsterdam)?;
        assert_eq!(gas_limit, mined_block_gas_limit);

        let gas_limit = send_transaction_without_gas(&provider)?;
        assert_eq!(gas_limit, mined_block_gas_limit);

        Ok(())
    }
}
