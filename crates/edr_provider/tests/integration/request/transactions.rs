use std::sync::Arc;

use anyhow::Context;
use edr_chain_l1::{rpc::transaction::L1RpcTransactionWithSignature, L1ChainSpec};
use edr_eth::{address, Address, Bytes, B256, U256};
use edr_provider::{
    test_utils::{create_test_config, one_ether},
    time::CurrentTime,
    MethodInvocation, NoopLogger, Provider, ProviderRequest,
};
use edr_rpc_eth::RpcTransactionRequest;
use edr_solidity::contract_decoder::ContractDecoder;
use tokio::runtime;

#[test]
fn transaction_by_hash_for_impersonated_account() -> anyhow::Result<()> {
    const IMPERSONATED_ACCOUNT: Address = address!("0x20620fa0ad46516e915029c94e3c87c9cd7861ff");

    let config = create_test_config();
    let chain_id = config.chain_id;

    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});
    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?;

    let _result = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::ImpersonateAccount(IMPERSONATED_ACCOUNT.into()),
    ))?;

    let _result = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SetBalance(IMPERSONATED_ACCOUNT, one_ether()),
    ))?;

    let _result = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::EvmSetAutomine(true),
    ))?;

    let result = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SendTransaction(RpcTransactionRequest {
            from: IMPERSONATED_ACCOUNT,
            to: Some(Address::ZERO),
            gas_price: Some(42_000_000_000),
            gas: Some(30_000),
            value: Some(U256::from(1)),
            data: Some(Bytes::default()),
            nonce: Some(0),
            chain_id: Some(chain_id),
            ..RpcTransactionRequest::default()
        }),
    ))?;

    let transaction_hash: B256 =
        serde_json::from_value(result.result).context("Failed to deserialize transaction hash")?;

    let result = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetTransactionByHash(transaction_hash),
    ))?;

    let found_transaction: L1RpcTransactionWithSignature =
        serde_json::from_value(result.result).context("Failed to deserialize transaction")?;

    assert_eq!(&found_transaction.from, &IMPERSONATED_ACCOUNT);
    assert_eq!(&found_transaction.hash, &transaction_hash);

    Ok(())
}
