use core::convert::Infallible;

use edr_eth::{
    address, bytes, eips::eip7702, signature::public_key_to_address,
    transaction::EthTransactionRequest, Bytes, SpecId, U256,
};
use edr_provider::{
    test_utils::{create_test_config, one_ether},
    AccountConfig, MethodInvocation, Provider, ProviderRequest,
};
use edr_rpc_eth::CallRequest;
use edr_test_utils::secret_key::{secret_key_from_str, SecretKey};

use crate::{assert_code_at, sign_authorization, CHAIN_ID};

static EXPECTED_CODE: Bytes = bytes!("ef01001234567890123456789012345678901234567890");

fn new_provider(sender_secret_key: SecretKey) -> anyhow::Result<Provider<Infallible>> {
    let mut config = create_test_config();
    config.accounts = vec![AccountConfig {
        secret_key: sender_secret_key,
        balance: one_ether(),
    }];
    config.chain_id = CHAIN_ID;
    config.hardfork = SpecId::PRAGUE;

    crate::new_provider(config)
}

fn signed_authorization(secret_key: &SecretKey) -> anyhow::Result<eip7702::SignedAuthorization> {
    sign_authorization(
        eip7702::Authorization {
            chain_id: U256::ZERO,
            address: address!("0x1234567890123456789012345678901234567890"),
            nonce: 0x1,
        },
        secret_key,
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn call() -> anyhow::Result<()> {
    let secret_key = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;
    let sender = public_key_to_address(secret_key.public_key());

    let call_request = CallRequest {
        from: Some(sender),
        to: Some(sender),
        authorization_list: Some(vec![signed_authorization(&secret_key)?]),
        ..CallRequest::default()
    };

    let provider = new_provider(secret_key)?;

    let _response = provider
        .handle_request(ProviderRequest::Single(MethodInvocation::Call(
            call_request,
            None,
            None,
        )))
        .expect("eth_call should succeed");

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn send_raw_transaction() -> anyhow::Result<()> {
    static RAW_TRANSACTION: Bytes = bytes!("0x04f8ca827a6980843b9aca00848321560082f61894f39fd6e51aad88f6f4ce6ab8827279cfffb922668080c0f85cf85a809412345678901234567890123456789012345678900101a02f97df52318e2bf310d3f9b823b0ca3b2e55b3bae9d82f025e68f04687810cb6a02d1a680365ebc7252024c7c5b43c2057e32bbef670398bb135f86e0dce225d6f01a066e35eed72225cd5d274b4a6ae5072bc245bd1c9664005e33a85ba217e6715dda014b070f113e9f887246981784ffd79865edcb30856551e5f385a39c7ea3170e3");

    let secret_key = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;
    let authorized_address = public_key_to_address(secret_key.public_key());

    let provider = new_provider(secret_key)?;
    let _response = provider
        .handle_request(ProviderRequest::Single(
            MethodInvocation::SendRawTransaction(RAW_TRANSACTION.clone()),
        ))
        .expect("eth_sendRawTransaction should succeed");

    assert_code_at(&provider, authorized_address, &EXPECTED_CODE);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn send_transaction() -> anyhow::Result<()> {
    let secret_key = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;
    let sender = public_key_to_address(secret_key.public_key());
    let authorized_address = sender;

    let transaction_request = EthTransactionRequest {
        chain_id: Some(CHAIN_ID),
        nonce: Some(0),
        from: sender,
        to: Some(sender),
        authorization_list: Some(vec![signed_authorization(&secret_key)?]),
        ..EthTransactionRequest::default()
    };

    let provider = new_provider(secret_key)?;

    let _response = provider
        .handle_request(ProviderRequest::Single(MethodInvocation::SendTransaction(
            transaction_request,
        )))
        .expect("eth_sendTransaction should succeed");

    assert_code_at(&provider, authorized_address, &EXPECTED_CODE);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn trace_call() -> anyhow::Result<()> {
    let secret_key = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;
    let sender = public_key_to_address(secret_key.public_key());

    let call_request = CallRequest {
        from: Some(sender),
        to: Some(sender),
        authorization_list: Some(vec![signed_authorization(&secret_key)?]),
        ..CallRequest::default()
    };

    let provider = new_provider(secret_key)?;

    let _response = provider
        .handle_request(ProviderRequest::Single(MethodInvocation::DebugTraceCall(
            call_request,
            None,
            None,
        )))
        .expect("debug_traceCall should succeed");

    Ok(())
}
