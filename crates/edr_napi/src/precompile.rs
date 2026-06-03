use edr_precompile::PrecompileFn;
use edr_primitives::Address;
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
    #[napi(catch_unwind, getter)]
    pub fn address(&self) -> Uint8Array {
        Uint8Array::with_data_copied(self.address)
    }
}

/// [RIP-7212](https://github.com/ethereum/RIPs/blob/master/RIPS/rip-7212.md#specification)
/// secp256r1 precompile.
#[napi(catch_unwind)]
pub fn precompile_p256_verify() -> Precompile {
    /// Wrapper function for the P256VERIFY precompile that calls
    /// [`edr_precompile::Precompile::execute`] on the `p256_verify` precompile.
    fn p256_verify_precompile_fn(
        input: &[u8],
        gas_limit: u64,
        reservoir: u64,
    ) -> edr_precompile::PrecompileResult {
        edr_precompile::secp256r1::P256VERIFY.execute(input, gas_limit, reservoir)
    }

    Precompile::new(
        *edr_precompile::secp256r1::P256VERIFY.address(),
        p256_verify_precompile_fn,
    )
}
