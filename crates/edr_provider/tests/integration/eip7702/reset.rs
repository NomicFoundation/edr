use edr_eth::{
    Address, Bytes, U256, address, bytes,
    eips::eip7702,
    l1::{self, L1ChainSpec},
    signature::public_key_to_address,
};
use edr_provider::{MethodInvocation, Provider, ProviderRequest, test_utils::create_test_config};
use edr_rpc_eth::TransactionRequest;
use edr_test_utils::secret_key::{SecretKey, secret_key_from_str};

use super::{CHAIN_ID, assert_code_at, sign_authorization};

static EXPECTED_CODE: Bytes = bytes!("ef01001234567890123456789012345678901234567890");

fn new_provider(sender_secret_key: SecretKey) -> anyhow::Result<Provider<L1ChainSpec>> {
    let mut config = create_test_config();
    config.chain_id = CHAIN_ID;
    config.hardfork = l1::SpecId::PRAGUE;

    super::new_provider(config, vec![sender_secret_key])
}

fn signed_authorization(
    address: Address,
    nonce: u64,
    secret_key: &SecretKey,
) -> anyhow::Result<eip7702::SignedAuthorization> {
    sign_authorization(
        eip7702::Authorization {
            chain_id: U256::from(CHAIN_ID),
            address,
            nonce,
        },
        secret_key,
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn send_raw_transaction() -> anyhow::Result<()> {
    static RAW_TRANSACTION1: Bytes = bytes!(
        "0x04f8cc827a6980843b9aca00848321560082f61894f39fd6e51aad88f6f4ce6ab8827279cfffb922668080c0f85ef85c827a699412345678901234567890123456789012345678900101a0eb775e0a2b7a15ea4938921e1ab255c84270e25c2c384b2adc32c73cd70273d6a046b9bec1961318a644db6cd9c7fc4e8d7c6f40d9165fc8958f3aff2216ed6f7c01a0be47a039954e4dfb7f08927ef7f072e0ec7510290e3c4c1405f3bf0329d0be51a06f291c455321a863d4c8ebbd73d58e809328918bcb5555958247ca6ec27feec8"
    );
    static RAW_TRANSACTION2: Bytes = bytes!(
        "0x04f8cc827a6902843b9aca00848321560082f61894f39fd6e51aad88f6f4ce6ab8827279cfffb922668080c0f85ef85c827a699400000000000000000000000000000000000000000380a06983300e20c4dadecfd39d5648fbd76e30ef9d3ebeee5f559b837a3fb95e339fa02b143d4c80182f623360f97a395f043ba715cc8f4b9780bb9055d903e410813b01a0d5d0729d6c57a9ca983131482c9c629859dedaaeba23ea0eedaa2da1376a71bba001f7456b3a4259421d4fa20ec4611d72549a57f65c83ba7251ee5c153c59a639"
    );

    let secret_key = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;
    let authorized_address = public_key_to_address(secret_key.public_key());

    let provider = new_provider(secret_key)?;
    let _response = provider
        .handle_request(ProviderRequest::Single(
            MethodInvocation::SendRawTransaction(RAW_TRANSACTION1.clone()),
        ))
        .expect("eth_sendRawTransaction should succeed");

    assert_code_at(&provider, authorized_address, &EXPECTED_CODE);

    let _response = provider
        .handle_request(ProviderRequest::Single(
            MethodInvocation::SendRawTransaction(RAW_TRANSACTION2.clone()),
        ))
        .expect("eth_sendRawTransaction should succeed");

    assert_code_at(&provider, authorized_address, &Bytes::new());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn send_transaction() -> anyhow::Result<()> {
    let secret_key = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;
    let sender = public_key_to_address(secret_key.public_key());
    let authorized_address = sender;

    let transaction_request1 = TransactionRequest {
        chain_id: Some(CHAIN_ID),
        nonce: Some(0),
        from: sender,
        to: Some(sender),
        authorization_list: Some(vec![signed_authorization(
            address!("0x1234567890123456789012345678901234567890"),
            0x1,
            &secret_key,
        )?]),
        ..TransactionRequest::default()
    };

    let transaction_request2 = TransactionRequest {
        chain_id: Some(CHAIN_ID),
        nonce: Some(2),
        from: sender,
        to: Some(sender),
        authorization_list: Some(vec![signed_authorization(Address::ZERO, 0x3, &secret_key)?]),
        ..TransactionRequest::default()
    };

    let provider = new_provider(secret_key)?;

    let _response = provider
        .handle_request(ProviderRequest::Single(MethodInvocation::SendTransaction(
            transaction_request1,
        )))
        .expect("eth_sendTransaction should succeed");

    assert_code_at(&provider, authorized_address, &EXPECTED_CODE);

    let _response = provider
        .handle_request(ProviderRequest::Single(MethodInvocation::SendTransaction(
            transaction_request2,
        )))
        .expect("eth_sendTransaction should succeed");

    assert_code_at(&provider, authorized_address, &Bytes::new());

    Ok(())
}
