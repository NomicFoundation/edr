use edr_chain_l1::{
    rpc::{call::L1CallRequest, TransactionRequest},
    L1ChainSpec,
};
use edr_eth::{address, bytes, Address, Bytes, U256};
use edr_provider::{test_utils::create_test_config, MethodInvocation, Provider, ProviderRequest};
use edr_signer::public_key_to_address;
use edr_test_utils::secret_key::{secret_key_from_str, SecretKey};

use super::{assert_code_at, sign_authorization, CHAIN_ID};

static EXPECTED_CODE1: Bytes = bytes!("ef01001234567890123456789012345678901234567890");
static EXPECTED_CODE2: Bytes = bytes!("ef01001111222233334444555566667777888899990000");

fn new_provider(sender_secret_key: SecretKey) -> anyhow::Result<Provider<L1ChainSpec>> {
    let mut config = create_test_config();
    config.chain_id = CHAIN_ID;
    config.hardfork = edr_chain_l1::Hardfork::PRAGUE;

    super::new_provider(config, vec![sender_secret_key])
}

fn signed_authorization(
    address: Address,
    secret_key: &SecretKey,
) -> anyhow::Result<edr_eip7702::SignedAuthorization> {
    sign_authorization(
        edr_eip7702::Authorization {
            chain_id: U256::from(CHAIN_ID),
            address,
            nonce: 0,
        },
        secret_key,
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn call() -> anyhow::Result<()> {
    let secret_key1 = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;
    let secret_key2 = secret_key_from_str(edr_defaults::SECRET_KEYS[1])?;

    let secret_key3 = secret_key_from_str(edr_defaults::SECRET_KEYS[2])?;
    let sender = public_key_to_address(secret_key3.public_key());

    let call_request = L1CallRequest {
        from: Some(sender),
        to: Some(sender),
        authorization_list: Some(vec![
            signed_authorization(
                address!("0x1234567890123456789012345678901234567890"),
                &secret_key1,
            )?,
            signed_authorization(
                address!("0x1111222233334444555566667777888899990000"),
                &secret_key2,
            )?,
        ]),
        ..L1CallRequest::default()
    };

    let provider = new_provider(secret_key3)?;

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
        "0x04f9012b827a6980843b9aca00848321560083019a2894f39fd6e51aad88f6f4ce6ab8827279cfffb922668080c0f8bcf85c827a699412345678901234567890123456789012345678908080a0b776080626e62615e2a51a6bde9b4b4612af2627e386734f9af466ecfce19b8da00d5c886f5874383826ac237ea99bfbbf601fad0fd344458296677930d51ff444f85c827a699411112222333344445555666677778888999900008080a0f71b006fda233cef9682622ca8e5f91f1764fd4d352e9a1240b2fcb6a118f3d8a06316d5485c65f186bb367250f229c9a612a40391df15d484e2acf499d9676f5e80a0ce08d98256e342d7b2d2286b3654ed976b5b6310015f45db34ccd07eb3c6b4daa00a09c71146122b7dfb21ce5276b6ed8e43006e0016cfe0cf89e87fb75ccae356"
    );

    let secret_key1 = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;
    let authorized_address1 = public_key_to_address(secret_key1.public_key());

    let secret_key2 = secret_key_from_str(edr_defaults::SECRET_KEYS[1])?;
    let authorized_address2 = public_key_to_address(secret_key2.public_key());

    let secret_key3 = secret_key_from_str(edr_defaults::SECRET_KEYS[2])?;

    let provider = new_provider(secret_key3)?;
    let _response = provider
        .handle_request(ProviderRequest::with_single(
            MethodInvocation::SendRawTransaction(RAW_TRANSACTION.clone()),
        ))
        .expect("eth_sendRawTransaction should succeed");

    assert_code_at(&provider, authorized_address1, &EXPECTED_CODE1);
    assert_code_at(&provider, authorized_address2, &EXPECTED_CODE2);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn send_transaction() -> anyhow::Result<()> {
    let secret_key1 = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;
    let authorized_address1 = public_key_to_address(secret_key1.public_key());

    let secret_key2 = secret_key_from_str(edr_defaults::SECRET_KEYS[1])?;
    let authorized_address2 = public_key_to_address(secret_key2.public_key());

    let secret_key3 = secret_key_from_str(edr_defaults::SECRET_KEYS[2])?;
    let sender = public_key_to_address(secret_key3.public_key());

    let transaction_request = TransactionRequest {
        chain_id: Some(CHAIN_ID),
        nonce: Some(0),
        from: sender,
        to: Some(sender),
        authorization_list: Some(vec![
            signed_authorization(
                address!("0x1234567890123456789012345678901234567890"),
                &secret_key1,
            )?,
            signed_authorization(
                address!("0x1111222233334444555566667777888899990000"),
                &secret_key2,
            )?,
        ]),
        ..TransactionRequest::default()
    };

    let provider = new_provider(secret_key3)?;

    let _response = provider
        .handle_request(ProviderRequest::with_single(
            MethodInvocation::SendTransaction(transaction_request),
        ))
        .expect("eth_sendTransaction should succeed");

    assert_code_at(&provider, authorized_address1, &EXPECTED_CODE1);
    assert_code_at(&provider, authorized_address2, &EXPECTED_CODE2);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn trace_call() -> anyhow::Result<()> {
    let secret_key1 = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;
    let secret_key2 = secret_key_from_str(edr_defaults::SECRET_KEYS[1])?;

    let secret_key3 = secret_key_from_str(edr_defaults::SECRET_KEYS[2])?;
    let sender = public_key_to_address(secret_key3.public_key());

    let call_request = L1CallRequest {
        from: Some(sender),
        to: Some(sender),
        authorization_list: Some(vec![
            signed_authorization(
                address!("0x1234567890123456789012345678901234567890"),
                &secret_key1,
            )?,
            signed_authorization(
                address!("0x1111222233334444555566667777888899990000"),
                &secret_key2,
            )?,
        ]),
        ..L1CallRequest::default()
    };

    let provider = new_provider(secret_key3)?;

    let _response = provider
        .handle_request(ProviderRequest::with_single(
            MethodInvocation::DebugTraceCall(call_request, None, None),
        ))
        .expect("debug_traceCall should succeed");

    Ok(())
}
