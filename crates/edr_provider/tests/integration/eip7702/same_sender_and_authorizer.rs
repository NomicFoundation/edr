use edr_chain_l1::{
    rpc::{call::L1CallRequest, TransactionRequest},
    L1ChainSpec,
};
use edr_primitives::{address, bytes, Bytes, U256};
use edr_provider::{test_utils::create_test_config, MethodInvocation, Provider, ProviderRequest};
use edr_signer::public_key_to_address;
use edr_test_utils::secret_key::{secret_key_from_str, SecretKey};

use super::{assert_code_at, sign_authorization, CHAIN_ID};

static EXPECTED_CODE: Bytes = bytes!("ef01001234567890123456789012345678901234567890");

fn new_provider(sender_secret_key: SecretKey) -> anyhow::Result<Provider<L1ChainSpec>> {
    let mut config = create_test_config();
    config.chain_id = CHAIN_ID;
    config.hardfork = edr_chain_l1::Hardfork::PRAGUE;

    super::new_provider(config, vec![sender_secret_key])
}

fn signed_authorization(
    secret_key: &SecretKey,
) -> anyhow::Result<edr_eip7702::SignedAuthorization> {
    sign_authorization(
        edr_eip7702::Authorization {
            chain_id: U256::from(CHAIN_ID),
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

    let call_request = L1CallRequest {
        from: Some(sender),
        to: Some(sender),
        authorization_list: Some(vec![signed_authorization(&secret_key)?]),
        ..L1CallRequest::default()
    };

    let provider = new_provider(secret_key)?;

    let _response = provider
        .handle_request(ProviderRequest::with_single(MethodInvocation::Call(
            call_request,
            None,
            None,
        )))
        .expect("eth_call should succeed");

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn send_raw_transaction() -> anyhow::Result<()> {
    static RAW_TRANSACTION: Bytes = bytes!(
        "0x04f8cc827a6980843b9aca00848321560082f61894f39fd6e51aad88f6f4ce6ab8827279cfffb922668080c0f85ef85c827a699412345678901234567890123456789012345678900101a0eb775e0a2b7a15ea4938921e1ab255c84270e25c2c384b2adc32c73cd70273d6a046b9bec1961318a644db6cd9c7fc4e8d7c6f40d9165fc8958f3aff2216ed6f7c01a0be47a039954e4dfb7f08927ef7f072e0ec7510290e3c4c1405f3bf0329d0be51a06f291c455321a863d4c8ebbd73d58e809328918bcb5555958247ca6ec27feec8"
    );

    let secret_key = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;
    let authorized_address = public_key_to_address(secret_key.public_key());

    let provider = new_provider(secret_key)?;
    let _response = provider
        .handle_request(ProviderRequest::with_single(
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

    let transaction_request = TransactionRequest {
        chain_id: Some(CHAIN_ID),
        nonce: Some(0),
        from: sender,
        to: Some(sender),
        authorization_list: Some(vec![signed_authorization(&secret_key)?]),
        ..TransactionRequest::default()
    };

    let provider = new_provider(secret_key)?;

    let _response = provider
        .handle_request(ProviderRequest::with_single(
            MethodInvocation::SendTransaction(transaction_request),
        ))
        .expect("eth_sendTransaction should succeed");

    assert_code_at(&provider, authorized_address, &EXPECTED_CODE);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn trace_call() -> anyhow::Result<()> {
    let secret_key = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;
    let sender = public_key_to_address(secret_key.public_key());

    let call_request = L1CallRequest {
        from: Some(sender),
        to: Some(sender),
        authorization_list: Some(vec![signed_authorization(&secret_key)?]),
        ..L1CallRequest::default()
    };

    let provider = new_provider(secret_key)?;

    let _response = provider
        .handle_request(ProviderRequest::with_single(
            MethodInvocation::DebugTraceCall(call_request, None, None),
        ))
        .expect("debug_traceCall should succeed");

    Ok(())
}
