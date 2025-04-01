use alloy_primitives::hex;
use napi::bindgen_prelude::{BigInt, Buffer};
use serde::Serializer;

/// Serialize a Buffer as a 0x-prefixed hex string
pub fn serialize_buffer_as_hex<S>(buffer: &Buffer, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let hex_string = format!("0x{}", hex::encode(buffer));
    serializer.serialize_str(&hex_string)
}

/// Serialize an Option<Buffer> as a 0x-prefixed hex string or None
pub fn serialize_optional_buffer_as_hex<S>(
    buffer: &Option<Buffer>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match buffer {
        Some(buf) => {
            let hex_string = format!("0x{}", hex::encode(buf));
            serializer.serialize_str(&hex_string)
        }
        None => serializer.serialize_none(),
    }
}

/// Serialize a `BigInt` as a struct with `sign_bit` and `words` fields
pub fn serialize_bigint_as_struct<S>(value: &BigInt, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    use serde::ser::SerializeStruct;

    let mut state = serializer.serialize_struct("BigInt", 2)?;
    state.serialize_field("sign_bit", &value.sign_bit)?;
    state.serialize_field("words", &value.words)?;
    state.end()
}

/// Serialize an Option<BigInt> as a struct with `sign_bit` and `words` fields
/// or None
pub fn serialize_optional_bigint_as_struct<S>(
    value: &Option<BigInt>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Some(val) => serialize_bigint_as_struct(val, serializer),
        None => serializer.serialize_none(),
    }
}
