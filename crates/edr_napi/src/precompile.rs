use edr_eth::Address;
use edr_evm::precompile::PrecompileFn;
use napi::bindgen_prelude::Uint8Array;
use napi_derive::napi;

#[napi]
#[derive(Clone)]
pub struct Precompile {
    address: Address,
    precompile_fn: PrecompileFn,
}

impl Precompile {
    pub fn new(address: Address, precompile_fn: PrecompileFn) -> Self {
        Self {
            address,
            precompile_fn,
        }
    }

    /// Returns the address and precompile function as a tuple.
    pub fn to_tuple(&self) -> (Address, PrecompileFn) {
        (self.address, self.precompile_fn)
    }
}

#[napi]
impl Precompile {
    /// Returns the address of the precompile.
    #[napi(getter)]
    pub fn address(&self) -> Uint8Array {
        Uint8Array::with_data_copied(self.address)
    }
}
