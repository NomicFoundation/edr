use napi::bindgen_prelude::Uint8Array;
use napi_derive::napi;

/// Computes the Keccak-256 hash of `data`, returning the 32-byte digest.
///
/// This is Ethereum's pre-standard Keccak-256, whose padding differs from the
/// standardized SHA3-256, so it can't be substituted by a platform SHA3
/// implementation. It is exposed so that JavaScript consumers can replace their
/// pure-JS implementations with EDR's native one.
// Deliberately not `catch_unwind`: hashing can't fail for any input, and the
// wrapper's cost is significant relative to hashing a 32-byte input, which is
// the common case for callers.
#[napi]
pub fn keccak256(data: Uint8Array) -> Uint8Array {
    let hash = edr_primitives::keccak256(data.as_ref());

    Uint8Array::new(hash.to_vec())
}
