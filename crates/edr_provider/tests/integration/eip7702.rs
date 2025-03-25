#![cfg(feature = "test-utils")]

mod different_sender_and_authorizer;
mod invalid_chain_id;
mod invalid_nonce;
mod multiple_authorizers;
mod reset;
mod same_sender_and_authorizer;
mod zeroed_chain_id;

use std::sync::Arc;

use edr_eth::{
    address,
    eips::eip7702,
    l1::{self, L1ChainSpec},
    signature::public_key_to_address,
    transaction::{self, ExecutableTransaction as _},
    Address, Bytes, B256, U256,
};
use edr_provider::{
    config::OwnedAccount,
    test_utils::{create_test_config, one_ether, sign_authorization},
    time::CurrentTime,
    MethodInvocation, NoopLogger, Provider, ProviderConfig, ProviderRequest,
};
use edr_rpc_eth::TransactionRequest;
use edr_solidity::contract_decoder::ContractDecoder;
use edr_test_utils::secret_key::secret_key_from_str;
use tokio::runtime;

const CHAIN_ID: u64 = 0x7a69;

fn assert_code_at(provider: &Provider<L1ChainSpec>, address: Address, expected: &Bytes) {
    let code: Bytes = {
        let response = provider
            .handle_request(ProviderRequest::Single(MethodInvocation::GetCode(
                address, None,
            )))
            .expect("eth_getCode should succeed");

        serde_json::from_value(response.result).expect("response should be Bytes")
    };

    assert_eq!(code, *expected);
}

fn new_provider(config: ProviderConfig<l1::SpecId>) -> anyhow::Result<Provider<L1ChainSpec>> {
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

    Ok(provider)
}

#[tokio::test(flavor = "multi_thread")]
async fn trace_transaction() -> anyhow::Result<()> {
    let secret_key = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;
    let sender = public_key_to_address(secret_key.public_key());

    let transaction_request = TransactionRequest {
        chain_id: Some(CHAIN_ID),
        nonce: Some(0),
        from: sender,
        to: Some(sender),
        authorization_list: Some(vec![sign_authorization(
            eip7702::Authorization {
                chain_id: U256::from(CHAIN_ID),
                address: address!("0x1234567890123456789012345678901234567890"),
                nonce: 0x1,
            },
            &secret_key,
        )?]),
        ..TransactionRequest::default()
    };

    let mut config = create_test_config();
    config.accounts = vec![OwnedAccount {
        secret_key,
        balance: one_ether(),
    }];
    config.chain_id = CHAIN_ID;
    config.hardfork = l1::SpecId::PRAGUE;

    let provider = new_provider(config)?;

    let response = provider
        .handle_request(ProviderRequest::Single(MethodInvocation::SendTransaction(
            transaction_request,
        )))
        .expect("eth_sendTransaction should succeed");

    let transaction_hash: B256 = serde_json::from_value(response.result)?;

    let _response = provider
        .handle_request(ProviderRequest::Single(
            MethodInvocation::DebugTraceTransaction(transaction_hash, None),
        ))
        .expect("debug_traceTransaction should succeed");

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn get_transaction() -> anyhow::Result<()> {
    let secret_key = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;
    let sender = public_key_to_address(secret_key.public_key());

    let transaction_request = TransactionRequest {
        chain_id: Some(CHAIN_ID),
        nonce: Some(0),
        from: sender,
        to: Some(sender),
        authorization_list: Some(vec![sign_authorization(
            eip7702::Authorization {
                chain_id: U256::from(CHAIN_ID),
                address: address!("0x1234567890123456789012345678901234567890"),
                nonce: 0x1,
            },
            &secret_key,
        )?]),
        ..TransactionRequest::default()
    };

    let mut config = create_test_config();
    config.accounts = vec![OwnedAccount {
        secret_key,
        balance: one_ether(),
    }];
    config.chain_id = CHAIN_ID;
    config.hardfork = l1::SpecId::PRAGUE;

    let provider = new_provider(config)?;

    let response = provider
        .handle_request(ProviderRequest::Single(MethodInvocation::SendTransaction(
            transaction_request.clone(),
        )))
        .expect("eth_sendTransaction should succeed");

    let transaction_hash: B256 = serde_json::from_value(response.result)?;

    let response = provider.handle_request(ProviderRequest::Single(
        MethodInvocation::GetTransactionByHash(transaction_hash),
    ))?;

    let transaction: edr_rpc_eth::TransactionWithSignature =
        serde_json::from_value(response.result)?;
    let transaction = transaction::Signed::try_from(transaction)?;

    if let transaction::Signed::Eip7702(transaction) = transaction {
        assert_eq!(Some(transaction.chain_id), transaction_request.chain_id);
        assert_eq!(Some(transaction.nonce), transaction_request.nonce);
        assert_eq!(*transaction.caller(), transaction_request.from);
        assert_eq!(Some(transaction.to), transaction_request.to);
        assert!(transaction.access_list.is_empty());
        assert_eq!(
            Some(transaction.authorization_list),
            transaction_request.authorization_list
        );
    } else {
        panic!("expected Eip7702 transaction. Found: {transaction:?}");
    }

    Ok(())
}
