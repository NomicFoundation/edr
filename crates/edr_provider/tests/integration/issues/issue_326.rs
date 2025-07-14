use std::sync::Arc;

use edr_chain_l1::{L1ChainSpec, L1Hardfork};
use edr_eth::Address;
use edr_provider::{
    test_utils::{create_test_config_with_fork, one_ether},
    time::CurrentTime,
    AccountOverride, MethodInvocation, MiningConfig, NoopLogger, Provider, ProviderRequest,
};
use edr_rpc_eth::{CallRequest, RpcTransactionRequest};
use edr_solidity::contract_decoder::ContractDecoder;
use tokio::runtime;

#[tokio::test(flavor = "multi_thread")]
async fn issue_326() -> anyhow::Result<()> {
    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});

    let mut config = create_test_config_with_fork(None);
    config.hardfork = L1Hardfork::CANCUN;
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
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?;

    provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::ImpersonateAccount(impersonated_account.into()),
    ))?;

    provider.handle_request(ProviderRequest::with_single(MethodInvocation::Mine(
        None, None,
    )))?;

    provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SendTransaction(RpcTransactionRequest {
            from: impersonated_account,
            to: Some(impersonated_account),
            nonce: Some(0),
            max_fee_per_gas: Some(0xA),
            ..RpcTransactionRequest::default()
        }),
    ))?;

    provider.handle_request(ProviderRequest::with_single(MethodInvocation::EstimateGas(
        CallRequest {
            from: Some(impersonated_account),
            to: Some(impersonated_account),
            max_fee_per_gas: Some(0x200),
            ..CallRequest::default()
        },
        None,
    )))?;

    Ok(())
}
