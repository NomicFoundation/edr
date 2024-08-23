#![cfg(feature = "test-utils")]

use edr_eth::chain_spec::L1ChainSpec;
use edr_provider::{
    test_utils::create_test_config, time::CurrentTime, MethodInvocation, NoopLogger, Sequential,
    ProviderRequest,
};
use tokio::runtime;

#[tokio::test(flavor = "multi_thread")]
async fn eth_max_priority_fee_per_gas() -> anyhow::Result<()> {
    let config = create_test_config();
    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});
    let provider = Sequential::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        CurrentTime,
    )?;

    let response = provider.handle_request(ProviderRequest::Single(
        MethodInvocation::MaxPriorityFeePerGas(()),
    ))?;

    // 1 gwei in hex
    assert_eq!(response.result, "0x3b9aca00");

    Ok(())
}
