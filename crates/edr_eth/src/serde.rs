//! Helper utilities for serde

use serde::{
    de::DeserializeOwned, ser::SerializeSeq, Deserialize, Deserializer, Serialize, Serializer,
};

/// for use with serde's `serialize_with` on an optional single value that
/// should be serialized as a sequence
pub fn optional_single_to_sequence<S, T>(val: &Option<T>, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: Serialize,
{
    let mut seq = s.serialize_seq(Some(1))?;
    if val.is_some() {
        seq.serialize_element(val)?;
    }
    seq.end()
}

/// for use with serde's `deserialize_with` on a sequence that should be
/// deserialized as a single but optional value.
pub fn sequence_to_optional_single<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Clone,
{
    let s: Vec<T> = Deserialize::deserialize(deserializer)?;
    if s.is_empty() {
        Ok(None)
    } else {
        Ok(Some(s[0].clone()))
    }
}

/// Deserialize a float vector. Needed to work around <https://github.com/serde-rs/json/issues/721.>
/// More context: <https://github.com/NomicFoundation/edr/pull/484>
pub fn deserialize_float_vec<'de, DeserializerT>(
    d: DeserializerT,
) -> Result<Vec<f64>, DeserializerT::Error>
where
    DeserializerT: Deserializer<'de>,
{
    Vec::<serde_json::Number>::deserialize(d)?
        .into_iter()
        .map(|n| {
            n.as_f64()
                .ok_or_else(|| serde::de::Error::custom("expected a finite float"))
        })
        .collect()
}

/// Helper module for optionally (de)serializing `[]` into `()`.
pub mod empty_params {
    use super::{Deserialize, Deserializer, Serialize, SerializeSeq, Serializer};

    /// Helper function for deserializing `[]` into `()`.
    pub fn deserialize<'de, DeserializerT>(d: DeserializerT) -> Result<(), DeserializerT::Error>
    where
        DeserializerT: Deserializer<'de>,
    {
        let seq = Option::<Vec<()>>::deserialize(d)?.unwrap_or_default();
        if !seq.is_empty() {
            return Err(serde::de::Error::custom(format!(
                "expected params sequence with length 0 but got {}",
                seq.len()
            )));
        }
        Ok(())
    }

    /// Helper function for serializing `()` into `[]`.
    pub fn serialize<SerializerT, T>(
        _val: &T,
        s: SerializerT,
    ) -> Result<SerializerT::Ok, SerializerT::Error>
    where
        SerializerT: Serializer,
        T: Serialize,
    {
        let seq = s.serialize_seq(Some(0))?;
        seq.end()
    }
}

/// Helper module for (de)serializing from/to a single value to/from a sequence.
pub mod sequence {
    use super::{Deserialize, DeserializeOwned, Deserializer, Serialize, SerializeSeq, Serializer};

    /// Helper function for deserializing a single value from a sequence.
    pub fn deserialize<'de, T, DeserializerT>(d: DeserializerT) -> Result<T, DeserializerT::Error>
    where
        DeserializerT: Deserializer<'de>,
        T: DeserializeOwned,
    {
        let mut seq = Vec::<T>::deserialize(d)?;
        if seq.len() != 1 {
            return Err(serde::de::Error::custom(format!(
                "expected params sequence with length 1 but got {}",
                seq.len()
            )));
        }
        Ok(seq.remove(0))
    }

    /// Helper function for serializing a single value into a sequence.
    pub fn serialize<SerializerT, T>(
        val: &T,
        s: SerializerT,
    ) -> Result<SerializerT::Ok, SerializerT::Error>
    where
        SerializerT: Serializer,
        T: Serialize,
    {
        let mut seq = s.serialize_seq(Some(1))?;
        seq.serialize_element(val)?;
        seq.end()
    }
}
