use core::str::FromStr;

use edr_defaults::SECRET_KEYS;
use hex::FromHexError;
use k256::SecretKey;

use crate::{
    address, b256,
    eips::eip7702,
    signature::{self, secret_key_from_str, SignatureError},
    transaction, Bytes, B256, U256,
};

const CHAIN_ID: u64 = 0x7a69;

/// Transaction hash for the test vector.
pub const TRANSACTION_HASH: B256 =
    b256!("235bb5a9856798eee27ec065a3aef0dc294a02713fce10c79321e436c98e1aab");

/// Signed authorization for the test vector.
pub fn signed_authorization() -> eip7702::SignedAuthorization {
    eip7702::SignedAuthorization::new_unchecked(
        eip7702::Authorization {
            chain_id: U256::from(CHAIN_ID),
            address: address!("0x1234567890123456789012345678901234567890"),
            nonce: 1,
        },
        1,
        U256::from_str("0xeb775e0a2b7a15ea4938921e1ab255c84270e25c2c384b2adc32c73cd70273d6")
            .expect("R value is valid"),
        U256::from_str("0x46b9bec1961318a644db6cd9c7fc4e8d7c6f40d9165fc8958f3aff2216ed6f7c")
            .expect("S value is valid"),
    )
}

/// Raw RLP-encoded signed transaction for the test vector.
pub fn raw() -> Result<Vec<u8>, FromHexError> {
    hex::decode(
                "04f8cc827a6980843b9aca00848321560082f61894f39fd6e51aad88f6f4ce6ab8827279cfffb922668080c0f85ef85c827a699412345678901234567890123456789012345678900101a0eb775e0a2b7a15ea4938921e1ab255c84270e25c2c384b2adc32c73cd70273d6a046b9bec1961318a644db6cd9c7fc4e8d7c6f40d9165fc8958f3aff2216ed6f7c01a0be47a039954e4dfb7f08927ef7f072e0ec7510290e3c4c1405f3bf0329d0be51a06f291c455321a863d4c8ebbd73d58e809328918bcb5555958247ca6ec27feec8",
            )
}

/// Test vector generated using secret key in `secret_key`.
pub fn request() -> anyhow::Result<transaction::request::Eip7702> {
    let request = transaction::request::Eip7702 {
        chain_id: CHAIN_ID,
        nonce: 0,
        max_priority_fee_per_gas: 1_000_000_000,
        max_fee_per_gas: 2_200_000_000,
        gas_limit: 63_000,
        to: address!("0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"),
        value: U256::ZERO,
        input: Bytes::new(),
        access_list: Vec::new(),
        authorization_list: vec![signed_authorization()],
    };
    Ok(request)
}

/// Test vector generated using secret key in `dummy_secret_key`.
pub fn signed() -> anyhow::Result<transaction::signed::Eip7702> {
    let request = request()?;

    let secret_key = secret_key()?;
    let signed = request.sign(&secret_key)?;

    Ok(signed)
}

/// Secret key used for the test vector.
pub fn secret_key() -> Result<SecretKey, SignatureError> {
    // This is test code, it's ok to use `DangerousSecretKeyStr`
    #[allow(deprecated)]
    secret_key_from_str(signature::DangerousSecretKeyStr(SECRET_KEYS[0]))
}
