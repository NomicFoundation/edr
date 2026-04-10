use std::sync::Arc;

use edr_chain_l1::{rpc::TransactionRequest, L1ChainSpec};
use edr_eth::{filter::LogFilterOptions, BlockSpec};
use edr_primitives::Address;
use edr_provider::{
    test_utils::{create_test_config_with, one_ether, MinimalProviderConfig},
    time::CurrentTime,
    handlers::{RpcMethodCall, RpcRequest},
    AccountOverride, NoopLogger, Provider,
};
use edr_solidity::contract_decoder::ContractDecoder;
use parking_lot::RwLock;
use tokio::runtime;

#[tokio::test(flavor = "multi_thread")]
async fn issue_361() -> anyhow::Result<()> {
    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});

    let mut config = create_test_config_with(MinimalProviderConfig::local_with_accounts());
    config.hardfork = edr_chain_l1::Hardfork::MUIR_GLACIER;

    let impersonated_account = Address::random();
    config.genesis_state.insert(
        impersonated_account,
        AccountOverride {
            balance: Some(one_ether()),
            ..AccountOverride::default()
        },
    );

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::new(RwLock::<ContractDecoder>::default()),
        CurrentTime,
    )?;

    provider.handle_request(RpcRequest::with_single(
        RpcMethodCall::with_params("hardhat_impersonateAccount", (impersonated_account,))?,
    ))?;

    provider.handle_request(RpcRequest::with_single(
        RpcMethodCall::with_params("eth_sendTransaction", (TransactionRequest {
            from: impersonated_account,
            to: Some(Address::random()),
            ..TransactionRequest::default()
        },))?,
    ))?;

    provider.handle_request(RpcRequest::with_single(
        RpcMethodCall::with_params("eth_getLogs", (LogFilterOptions {
            from_block: Some(BlockSpec::Number(0)),
            to_block: Some(BlockSpec::latest()),
            ..LogFilterOptions::default()
        },))?,
    ))?;

    Ok(())
}
