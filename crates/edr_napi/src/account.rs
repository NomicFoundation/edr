use napi::bindgen_prelude::{BigInt, Uint8Array};
use napi_derive::napi;

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
