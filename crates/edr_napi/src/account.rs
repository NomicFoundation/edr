use napi::bindgen_prelude::{BigInt, Uint8Array};
use napi_derive::napi;

/// Specification of overrides for an account and its storage.
#[napi(object)]
pub struct AccountOverride {
    /// The account's address
    pub address: Uint8Array,
    /// If present, the overwriting balance.
    pub balance: Option<BigInt>,
    /// If present, the overwriting nonce.
    pub nonce: Option<BigInt>,
    /// If present, the overwriting code.
    pub code: Option<Uint8Array>,
    /// BEWARE: This field is not supported yet. See <https://github.com/NomicFoundation/edr/issues/911>
    ///
    /// If present, the overwriting storage.
    pub storage: Option<Vec<StorageSlot>>,
}

/// A description of a storage slot's state.
#[napi(object)]
pub struct StorageSlot {
    /// The storage slot's index
    pub index: BigInt,
    /// The storage slot's value
    pub value: BigInt,
}
