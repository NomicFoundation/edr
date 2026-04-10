use std::sync::Arc;

use edr_chain_l1::{
    rpc::{call::L1CallRequest, TransactionRequest},
    L1ChainSpec,
};
use edr_primitives::Address;
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
async fn issue_326() -> anyhow::Result<()> {
    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});

    let mut config = create_test_config_with(MinimalProviderConfig::local_with_accounts());
    config.hardfork = edr_chain_l1::Hardfork::CANCUN;
    config.mining = MiningConfig {
        auto_mine: false,
        ..MiningConfig::default()
    };
    config.initial_base_fee_per_gas = Some(0x100);

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
        RpcMethodCall::with_params("hardhat_mine", (Option::<u64>::None, Option::<u64>::None))?,
    ))?;

    provider.handle_request(RpcRequest::with_single(
        RpcMethodCall::with_params("eth_sendTransaction", (TransactionRequest {
            from: impersonated_account,
            to: Some(impersonated_account),
            nonce: Some(0),
            max_fee_per_gas: Some(0xA),
            ..TransactionRequest::default()
        },))?,
    ))?;

    provider.handle_request(RpcRequest::with_single(
        RpcMethodCall::with_params("eth_estimateGas", (L1CallRequest {
            from: Some(impersonated_account),
            to: Some(impersonated_account),
            max_fee_per_gas: Some(0x200),
            ..L1CallRequest::default()
        }, Option::<edr_eth::BlockSpec>::None))?,
    ))?;

    Ok(())
}
