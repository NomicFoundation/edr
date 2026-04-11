use std::sync::Arc;

use edr_op::OpChainSpec;
use edr_provider::{
    handlers::{RpcMethodCall, RpcRequest},
    test_utils::create_test_config, time::CurrentTime, NoopLogger, Provider,
};
use edr_solidity::contract_decoder::ContractDecoder;
use parking_lot::RwLock;
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
        Arc::new(RwLock::<ContractDecoder>::default()),
        CurrentTime,
    )?;

    // Mine a block to make sure that the genesis block uses the correct extra
    // data, containing dynamic base fee params.
    let _response = provider.handle_request(RpcRequest::with_single(
        RpcMethodCall::with_params("hardhat_mine", (Option::<u64>::None, Option::<u64>::None))?,
    ))?;

    Ok(())
}
