use k256::{
    elliptic_curve::{ops::MulByGenerator, sec1::ToEncodedPoint},
    NonZeroScalar, ProjectivePoint,
};
use napi::bindgen_prelude::Uint8Array;
use napi_derive::napi;

/// Derives the secp256k1 public key of the provided secret key, returning it
/// in uncompressed SEC1 form: 65 bytes, `0x04 || X || Y`.
///
/// Throws if the input isn't a valid secret key: exactly 32 bytes encoding a
/// big-endian scalar in `[1, n)`, where `n` is the curve order.
// The `js_name` avoids the `secp256K1` that the automatic camel-casing of
// `secp256k1_` would produce.
#[napi(js_name = "secp256k1PublicKeyFromSecretKey")]
pub fn secp256k1_public_key_from_secret_key(secret_key: Uint8Array) -> napi::Result<Uint8Array> {
    let scalar = NonZeroScalar::try_from(secret_key.as_ref()).map_err(|_error| {
        napi::Error::new(
            napi::Status::InvalidArg,
            "Expected a 32-byte big-endian scalar in [1, n)",
        )
    })?;

    // `mul_by_generator` uses the fixed-base precomputed table, unlike
    // `SecretKey::public_key`, which performs a generic variable-base
    // multiplication.
    let public_key = ProjectivePoint::mul_by_generator(&*scalar).to_affine();

    Ok(Uint8Array::new(
        public_key.to_encoded_point(false).as_bytes().to_vec(),
    ))
}
