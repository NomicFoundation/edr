#![cfg(feature = "test-utils")]

use std::{
    num::NonZeroU64,
    sync::Arc,
    time::{Duration, Instant},
};

use edr_chain_l1::L1ChainSpec;
use edr_primitives::U256;
use edr_provider::{
    config::{IntervalConfig, IntervalRangeConfig},
    test_utils::create_test_config,
    time::CurrentTime,
    IntervalConfigRequest, MethodInvocation, NoopLogger, Provider, ProviderRequest,
};
use edr_solidity::contract_decoder::ContractDecoder;
use parking_lot::RwLock;
use tokio::runtime;

const INTERVAL_MS: u64 = 50;
/// Generous relative to [`INTERVAL_MS`], so the assertions do not depend on
/// exact timing.
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
    let block_number: U256 = response.deserialize_result()?;
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
    let provider = provider_with_interval(None)?;

    let start = block_number(&provider)?;
    std::thread::sleep(Duration::from_millis(INTERVAL_MS * 4));
    assert_eq!(
        block_number(&provider)?,
        start,
        "no blocks should be mined while interval mining is disabled"
    );

    set_interval_mining(
        &provider,
        IntervalConfigRequest::FixedOrDisabled(INTERVAL_MS),
    )?;
    assert!(
        wait_for_block_after(&provider, start)?,
        "enabling interval mining should produce a new block"
    );

    // The disable request is handled by the mining thread, so no block can be
    // mined after it returns.
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

/// A range interval arms the timer just as a fixed one does.
#[tokio::test(flavor = "multi_thread")]
async fn interval_mining_with_a_range_mines_blocks() -> anyhow::Result<()> {
    let range = IntervalRangeConfig::try_from([INTERVAL_MS, 2 * INTERVAL_MS])?;
    let provider = provider_with_interval(Some(IntervalConfig::Range(range)))?;

    let start = block_number(&provider)?;
    assert!(
        wait_for_block_after(&provider, start)?,
        "interval mining should produce a new block"
    );

    Ok(())
}

/// An invalid range is rejected, and rejecting it leaves the provider usable.
/// Before the range was validated, `[2000, 1000]` panicked the thread that owns
/// the provider's data on the first interval-mined block.
#[tokio::test(flavor = "multi_thread")]
async fn evm_set_interval_mining_rejects_an_invalid_range() -> anyhow::Result<()> {
    let provider = provider_with_interval(None)?;

    for bounds in [[2000, 1000], [0, 0], [0, 2000]] {
        set_interval_mining(&provider, IntervalConfigRequest::Range(bounds))
            .expect_err("the range is invalid");
    }

    // The provider still answers, and no interval mining was configured.
    let start = block_number(&provider)?;
    std::thread::sleep(Duration::from_millis(INTERVAL_MS * 4));
    assert_eq!(block_number(&provider)?, start);

    Ok(())
}

/// `evm_setIntervalMining` restarts the timer even when it sets the interval
/// that is already configured, so the next block is always due a full interval
/// after the call.
#[tokio::test(flavor = "multi_thread")]
async fn evm_set_interval_mining_restarts_on_an_unchanged_interval() -> anyhow::Result<()> {
    /// Long enough that no block is ever due between two consecutive requests,
    /// short enough that a timer left alone certainly fires within
    /// [`OBSERVATION`].
    const RESTART_INTERVAL_MS: u64 = 1_000;
    /// Twenty requests per interval, so a spurious block needs a one-second
    /// stall where fifty milliseconds were asked for.
    const RESET_PERIOD: Duration = Duration::from_millis(50);
    /// Three intervals, so a timer that is not restarted fires three times.
    const OBSERVATION: Duration = Duration::from_millis(3 * RESTART_INTERVAL_MS);

    let provider = provider_with_interval(None)?;
    let start = block_number(&provider)?;

    let deadline = Instant::now() + OBSERVATION;
    while Instant::now() < deadline {
        set_interval_mining(
            &provider,
            IntervalConfigRequest::FixedOrDisabled(RESTART_INTERVAL_MS),
        )?;
        std::thread::sleep(RESET_PERIOD);
    }

    assert_eq!(
        block_number(&provider)?,
        start,
        "every request should have restarted the timer, so no block was ever due"
    );

    // Not vacuous: the timer was armed throughout and fires once the interval
    // is left alone.
    assert!(
        wait_for_block_after(&provider, start)?,
        "interval mining should resume once the interval is no longer reset"
    );

    Ok(())
}
