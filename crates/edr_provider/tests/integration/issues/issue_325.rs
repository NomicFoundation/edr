use std::sync::Arc;

use edr_chain_l1::{rpc::TransactionRequest, L1ChainSpec};
use edr_eth::PreEip1898BlockSpec;
use edr_primitives::{Address, B256};
use edr_provider::{
    test_utils::{create_test_config_with, one_ether, MinimalProviderConfig},
    time::CurrentTime,
    handlers::{RpcMethodCall, RpcRequest},
    AccountOverride, MiningConfig, NoopLogger, Provider,
};
use edr_solidity::contract_decoder::ContractDecoder;
use parking_lot::RwLock;
use tokio::runtime;

#[tokio::test(flavor = "multi_thread")]
async fn issue_325() -> anyhow::Result<()> {
    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});

    let mut config = create_test_config_with(MinimalProviderConfig::local_with_accounts());
    config.hardfork = edr_chain_l1::Hardfork::CANCUN;
    config.mining = MiningConfig {
        auto_mine: false,
        ..MiningConfig::default()
    };

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

    let result = provider.handle_request(RpcRequest::with_single(
        RpcMethodCall::with_params("eth_sendTransaction", (TransactionRequest {
            from: impersonated_account,
            to: Some(Address::random()),
            ..TransactionRequest::default()
        },))?,
    ))?;

    let transaction_hash: B256 = serde_json::from_value(result.result)?;

    let result = provider.handle_request(RpcRequest::with_single(
        RpcMethodCall::with_params("hardhat_dropTransaction", (transaction_hash,))?,
    ))?;

    let dropped: bool = serde_json::from_value(result.result)?;

    assert!(dropped);

    provider.handle_request(RpcRequest::with_single(
        RpcMethodCall::with_params("eth_getBlockByNumber", (PreEip1898BlockSpec::pending(), false))?,
    ))?;

    Ok(())
}
