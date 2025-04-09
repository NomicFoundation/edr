use core::fmt::{Debug, Display};

use edr_eth::signature::secret_key_from_str;
use napi::{
    JsString, Status,
    bindgen_prelude::{BigInt, Uint8Array},
};
use napi_derive::napi;
use serde::Serialize;

use crate::cast::TryCast as _;

/// A description of an account's state.
#[napi(object)]
pub struct Account {
    /// The account's address
    pub address: Uint8Array,
    /// The account's balance
    pub balance: BigInt,
    /// The account's nonce
    pub nonce: BigInt,
    /// The account's code
    pub code: Option<Uint8Array>,
    /// The account's storage
    pub storage: Vec<StorageSlot>,
}

/// A description of a storage slot's state.
#[napi(object)]
pub struct StorageSlot {
    /// The storage slot's index
    pub index: BigInt,
    /// The storage slot's value
    pub value: BigInt,
}

/// An owned account, for which the secret key is known, and its desired genesis
/// balance.
#[napi(object)]
pub struct OwnedAccount {
    // Using JsString here as it doesn't have `Debug`, `Display` and `Serialize` implementation
    // which prevents accidentally leaking the secret keys to error messages and logs.
    /// Account secret key
    pub secret_key: JsString,
    /// Account balance
    pub balance: BigInt,
}

impl TryFrom<OwnedAccount> for edr_provider::config::OwnedAccount {
    type Error = napi::Error;

    fn try_from(value: OwnedAccount) -> Result<Self, Self::Error> {
        // This is the only place in production code where it's allowed to use
        // `DangerousSecretKeyStr`.
        #[allow(deprecated)]
        use edr_eth::signature::DangerousSecretKeyStr;

        static_assertions::assert_not_impl_all!(JsString: Debug, Display, Serialize);
        // `k256::SecretKey` has `Debug` implementation, but it's opaque (only shows the
        // type name)
        static_assertions::assert_not_impl_any!(k256::SecretKey: Display, Serialize);

        let secret_key = value.secret_key.into_utf8()?;
        // This is the only place in production code where it's allowed to use
        // `DangerousSecretKeyStr`.
        #[allow(deprecated)]
        let secret_key_str = DangerousSecretKeyStr(secret_key.as_str()?);
        let secret_key: k256::SecretKey = secret_key_from_str(secret_key_str)
            .map_err(|e| napi::Error::new(Status::InvalidArg, e.to_string()))?;

        Ok(Self {
            secret_key,
            balance: value.balance.try_cast()?,
        })
    }
}
