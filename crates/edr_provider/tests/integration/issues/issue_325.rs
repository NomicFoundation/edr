use std::sync::Arc;

use edr_eth::{
    l1::{self, L1ChainSpec},
    Address, PreEip1898BlockSpec, B256,
};
use edr_provider::{
    test_utils::{create_test_config_with_fork, one_ether},
    time::CurrentTime,
    AccountOverride, MethodInvocation, MiningConfig, NoopLogger, Provider, ProviderRequest,
};
use edr_rpc_eth::TransactionRequest;
use edr_solidity::contract_decoder::ContractDecoder;
use tokio::runtime;

#[tokio::test(flavor = "multi_thread")]
async fn issue_325() -> anyhow::Result<()> {
    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});

    let mut config = create_test_config_with_fork(None);
    config.hardfork = l1::SpecId::CANCUN;
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
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?;

    provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::ImpersonateAccount(impersonated_account.into()),
    ))?;

    let result = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SendTransaction(TransactionRequest {
            from: impersonated_account,
            to: Some(Address::random()),
            ..TransactionRequest::default()
        }),
    ))?;

    let transaction_hash: B256 = serde_json::from_value(result.result)?;

    let result = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::DropTransaction(transaction_hash),
    ))?;

    let dropped: bool = serde_json::from_value(result.result)?;

    assert!(dropped);

    provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::pending(), false),
    ))?;

    Ok(())
}
