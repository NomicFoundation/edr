use std::sync::Arc;

use edr_op::OpChainSpec;
use edr_provider::{
    test_utils::create_test_config, time::CurrentTime, MethodInvocation, NoopLogger, Provider,
    ProviderRequest,
};
use edr_solidity::contract_decoder::ContractDecoder;
use tokio::runtime;

#[tokio::test(flavor = "multi_thread")]
async fn holocene_genesis_block() -> anyhow::Result<()> {
    let logger = Box::new(NoopLogger::<OpChainSpec>::default());
    let subscriber = Box::new(|_event| {});

    let mut config = create_test_config();
    config.hardfork = edr_op::Hardfork::HOLOCENE;

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?;

    // Mine a block to make sure that the genesis block uses the correct extra
    // data, containing dynamic base fee params.
    let _response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::Mine(None, None),
    ))?;

    Ok(())
}
