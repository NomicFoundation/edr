use std::sync::Arc;

use edr_chain_l1::{rpc::TransactionRequest, L1ChainSpec};
use edr_eth::{filter::LogFilterOptions, BlockSpec};
use edr_primitives::Address;
use edr_provider::{
    test_utils::{create_test_config_with_fork, one_ether},
    time::CurrentTime,
    AccountOverride, MethodInvocation, NoopLogger, Provider, ProviderRequest,
};
use edr_solidity::contract_decoder::ContractDecoder;
use tokio::runtime;

#[tokio::test(flavor = "multi_thread")]
async fn issue_361() -> anyhow::Result<()> {
    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});

    let mut config = create_test_config_with_fork(None);
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
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?;

    provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::ImpersonateAccount(impersonated_account.into()),
    ))?;

    provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SendTransaction(TransactionRequest {
            from: impersonated_account,
            to: Some(Address::random()),
            ..TransactionRequest::default()
        }),
    ))?;

    provider.handle_request(ProviderRequest::with_single(MethodInvocation::GetLogs(
        LogFilterOptions {
            from_block: Some(BlockSpec::Number(0)),
            to_block: Some(BlockSpec::latest()),
            ..LogFilterOptions::default()
        },
    )))?;

    Ok(())
}
