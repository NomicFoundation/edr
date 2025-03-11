use core::convert::Infallible;

use edr_eth::{
    address, bytes, eips::eip7702, signature::public_key_to_address,
    transaction::EthTransactionRequest, Bytes, SpecId, U256,
};
use edr_provider::{
    test_utils::{create_test_config, one_ether},
    AccountConfig, MethodInvocation, Provider, ProviderRequest,
};
use edr_test_utils::secret_key::{secret_key_from_str, SecretKey};

use super::{assert_code_at, sign_authorization, CHAIN_ID};

fn new_provider(sender_secret_key: SecretKey) -> anyhow::Result<Provider<Infallible>> {
    let mut config = create_test_config();
    config.accounts = vec![AccountConfig {
        secret_key: sender_secret_key,
        balance: one_ether(),
    }];
    config.chain_id = CHAIN_ID;
    config.hardfork = SpecId::PRAGUE;

    super::new_provider(config)
}

fn signed_authorization(secret_key: &SecretKey) -> anyhow::Result<eip7702::SignedAuthorization> {
    sign_authorization(
        eip7702::Authorization {
            chain_id: U256::from(0x1),
            address: address!("0x1234567890123456789012345678901234567890"),
            nonce: 0x1,
        },
        secret_key,
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn send_raw_transaction() -> anyhow::Result<()> {
    static RAW_TRANSACTION: Bytes = bytes!("0x04f8ce827a6980843b9aca00848321560082f61894f39fd6e51aad88f6f4ce6ab8827279cfffb922668080c0f860f85e827a6994123456789012345678901234567890123456789082010080a03470be5b900881063c9b307a5e45878ab3c89c47dca071830290a85e2a2810d9a01b0fd2b15c1add6bc2b3670b1f7d2593a1c4db15e0c1f218085afdbde765275280a0753727e98b92b846d67bb893ebe1f0e95b743dc688e5a657a56951feb63cb5d8a065615fc4172d3e89ffdda85fe0dc454e82ca431811c7e2c83702f83ecb2edb84");

    let secret_key = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;
    let authorized_address = public_key_to_address(secret_key.public_key());

    let provider = new_provider(secret_key)?;
    let _response = provider
        .handle_request(ProviderRequest::Single(
            MethodInvocation::SendRawTransaction(RAW_TRANSACTION.clone()),
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

    assert_code_at(&provider, authorized_address, &Bytes::new());

    Ok(())
}
