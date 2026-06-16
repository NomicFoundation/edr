#![cfg(feature = "test-utils")]

use std::{
    num::NonZeroU64,
    sync::Arc,
    time::{Duration, Instant},
};

use edr_chain_l1::L1ChainSpec;
use edr_primitives::U256;
use edr_provider::{
    config::IntervalConfig, test_utils::create_test_config, time::CurrentTime,
    IntervalConfigRequest, MethodInvocation, NoopLogger, Provider, ProviderRequest,
};
use edr_solidity::contract_decoder::ContractDecoder;
use parking_lot::RwLock;
use tokio::runtime;

/// A short interval keeps the tests fast while remaining robust: assertions
/// poll for up to [`POLL_TIMEOUT`] rather than relying on exact timing.
const INTERVAL_MS: u64 = 50;
const POLL_TIMEOUT: Duration = Duration::from_secs(5);

fn provider_with_interval(
    interval: Option<IntervalConfig>,
) -> anyhow::Result<Provider<L1ChainSpec>> {
    let logger = Box::<NoopLogger<L1ChainSpec>>::default();
    let subscription_callback_noop = Box::new(|_| ());

    let mut config = create_test_config();
    config.mining.interval = interval;

    Ok(Provider::new(
        runtime::Handle::current(),
        logger,
        subscription_callback_noop,
        config,
        Arc::new(RwLock::<ContractDecoder>::default()),
        CurrentTime,
    )?)
}

fn block_number(provider: &Provider<L1ChainSpec>) -> anyhow::Result<u64> {
    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::BlockNumber(()),
    ))?;
    let block_number: U256 = serde_json::from_value(response.result)?;
    Ok(block_number.to::<u64>())
}

fn set_interval_mining(
    provider: &Provider<L1ChainSpec>,
    config: IntervalConfigRequest,
) -> anyhow::Result<()> {
    provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::EvmSetIntervalMining(config),
    ))?;
    Ok(())
}

/// Polls until the block number exceeds `from`, returning `false` on timeout.
fn wait_for_block_after(provider: &Provider<L1ChainSpec>, from: u64) -> anyhow::Result<bool> {
    let deadline = Instant::now() + POLL_TIMEOUT;
    loop {
        if block_number(provider)? > from {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn interval_mining_mines_blocks() -> anyhow::Result<()> {
    let interval = IntervalConfig::Fixed(NonZeroU64::new(INTERVAL_MS).expect("non-zero"));
    let provider = provider_with_interval(Some(interval))?;

    let start = block_number(&provider)?;
    assert!(
        wait_for_block_after(&provider, start)?,
        "interval mining should produce a new block"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn evm_set_interval_mining_enables_and_disables() -> anyhow::Result<()> {
    // Interval mining is disabled initially: no blocks should be produced.
    let provider = provider_with_interval(None)?;

    let start = block_number(&provider)?;
    std::thread::sleep(Duration::from_millis(INTERVAL_MS * 4));
    assert_eq!(
        block_number(&provider)?,
        start,
        "no blocks should be mined while interval mining is disabled"
    );

    // Enable interval mining at runtime and confirm blocks start appearing.
    set_interval_mining(&provider, IntervalConfigRequest::FixedOrDisabled(INTERVAL_MS))?;
    assert!(
        wait_for_block_after(&provider, start)?,
        "enabling interval mining should produce a new block"
    );

    // Disable interval mining again. The disable request is processed by the
    // same thread that mines, so once it returns no further blocks are mined.
    set_interval_mining(&provider, IntervalConfigRequest::FixedOrDisabled(0))?;
    let after_disable = block_number(&provider)?;
    std::thread::sleep(Duration::from_millis(INTERVAL_MS * 4));
    assert_eq!(
        block_number(&provider)?,
        after_disable,
        "no blocks should be mined after disabling interval mining"
    );

    Ok(())
}
