use edr_eth::{
    Bytes, U256, address, bytes,
    eips::eip7702,
    l1::{self, L1ChainSpec},
    signature::public_key_to_address,
};
use edr_provider::{
    MethodInvocation, Provider, ProviderRequest,
    config::OwnedAccount,
    test_utils::{create_test_config, one_ether},
};
use edr_rpc_eth::{CallRequest, TransactionRequest};
use edr_test_utils::secret_key::{SecretKey, secret_key_from_str};

use super::{CHAIN_ID, assert_code_at, sign_authorization};

static EXPECTED_CODE: Bytes = bytes!("ef01001234567890123456789012345678901234567890");

fn new_provider(sender_secret_key: SecretKey) -> anyhow::Result<Provider<L1ChainSpec>> {
    let mut config = create_test_config();
    config.accounts = vec![OwnedAccount {
        secret_key: sender_secret_key,
        balance: one_ether(),
    }];
    config.chain_id = CHAIN_ID;
    config.hardfork = l1::SpecId::PRAGUE;

    super::new_provider(config)
}

fn signed_authorization(secret_key: &SecretKey) -> anyhow::Result<eip7702::SignedAuthorization> {
    sign_authorization(
        eip7702::Authorization {
            chain_id: U256::from(CHAIN_ID),
            address: address!("0x1234567890123456789012345678901234567890"),
            nonce: 0x0,
        },
        secret_key,
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn call() -> anyhow::Result<()> {
    let secret_key1 = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;

    let secret_key2 = secret_key_from_str(edr_defaults::SECRET_KEYS[1])?;
    let sender = public_key_to_address(secret_key2.public_key());

    let call_request = CallRequest {
        from: Some(sender),
        to: Some(sender),
        authorization_list: Some(vec![signed_authorization(&secret_key1)?]),
        ..CallRequest::default()
    };

    let provider = new_provider(secret_key2)?;

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
    static RAW_TRANSACTION: Bytes = bytes!(
        "0x04f8cc827a6980843b9aca00848321560082f61894f39fd6e51aad88f6f4ce6ab8827279cfffb922668080c0f85ef85c827a699412345678901234567890123456789012345678908080a0b776080626e62615e2a51a6bde9b4b4612af2627e386734f9af466ecfce19b8da00d5c886f5874383826ac237ea99bfbbf601fad0fd344458296677930d51ff44480a0a5f83207382081e8de07113af9ba61e4b41c9ae306edc55a2787996611d1ade9a0082f979b985ea64b4755344b57bcd66ade2b840e8be2036101d9cf23a8548412"
    );

    let secret_key1 = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;
    let authorized_address = public_key_to_address(secret_key1.public_key());

    let secret_key2 = secret_key_from_str(edr_defaults::SECRET_KEYS[1])?;
    let provider = new_provider(secret_key2)?;

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
    let secret_key1 = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;
    let authorized_address = public_key_to_address(secret_key1.public_key());

    let secret_key2 = secret_key_from_str(edr_defaults::SECRET_KEYS[1])?;
    let sender = public_key_to_address(secret_key2.public_key());

    let transaction_request = TransactionRequest {
        chain_id: Some(CHAIN_ID),
        nonce: Some(0),
        from: sender,
        to: Some(sender),
        authorization_list: Some(vec![signed_authorization(&secret_key1)?]),
        ..TransactionRequest::default()
    };

    let provider = new_provider(secret_key2)?;

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
    let secret_key1 = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;

    let secret_key2 = secret_key_from_str(edr_defaults::SECRET_KEYS[1])?;
    let sender = public_key_to_address(secret_key2.public_key());

    let call_request = CallRequest {
        from: Some(sender),
        to: Some(sender),
        authorization_list: Some(vec![signed_authorization(&secret_key1)?]),
        ..CallRequest::default()
    };

    let provider = new_provider(secret_key2)?;

    let _response = provider
        .handle_request(ProviderRequest::Single(MethodInvocation::DebugTraceCall(
            call_request,
            None,
            None,
        )))
        .expect("debug_traceCall should succeed");

    Ok(())
}
