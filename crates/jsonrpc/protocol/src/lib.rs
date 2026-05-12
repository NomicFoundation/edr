#![warn(missing_docs)]
//! JSON-RPC 2.0 protocol types.

// Parts of this code were adapted from github.com/koushiro/async-jsonrpc and
// are distributed under its licenses:
// - https://github.com/koushiro/async-jsonrpc/blob/9b42602f4faa63dd4b6a1a9fe359bffa97e636d5/LICENSE-APACHE
// - https://github.com/koushiro/async-jsonrpc/blob/9b42602f4faa63dd4b6a1a9fe359bffa97e636d5/LICENSE-MIT
// For the original context, see https://github.com/koushiro/async-jsonrpc/tree/9b42602f4faa63dd4b6a1a9fe359bffa97e636d5

use core::marker::PhantomData;

use serde::{Deserialize, Serialize};

/// Standard JSON-RPC 2.0 "Internal error" code.
pub const INTERNAL_ERROR_CODE: i16 = -32603;

/// Standard JSON-RPC 2.0 "Invalid params" code.
pub const INVALID_PARAMS_CODE: i16 = -32602;

/// Represents either a single JSON-RPC request/response or a batch of them.
pub enum SingleOrBatch<T> {
    /// A single request/response.
    Single(T),
    /// A batch of requests/responses.
    Batch(Vec<T>),
}

// Custom deserializer instead of using `#[serde(untagged)]` as it hides custom
// error messages which are important to propagate to users.
impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for SingleOrBatch<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct SingleOrBatchVisitor<T>(PhantomData<T>);

        impl<'de, T: serde::Deserialize<'de>> serde::de::Visitor<'de> for SingleOrBatchVisitor<T> {
            type Value = SingleOrBatch<T>;

            fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str("a single JSON-RPC object or an array of them")
            }

            fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                // Forward to deserializer of `Vec<T>`
                let requests = serde::Deserialize::deserialize(
                    serde::de::value::SeqAccessDeserializer::new(seq),
                )?;

                Ok(SingleOrBatch::Batch(requests))
            }

            fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let request = T::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                Ok(SingleOrBatch::Single(request))
            }
        }

        deserializer.deserialize_any(SingleOrBatchVisitor(PhantomData))
    }
}

impl<T: serde::Serialize> serde::Serialize for SingleOrBatch<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            SingleOrBatch::Single(item) => item.serialize(serializer),
            SingleOrBatch::Batch(items) => items.serialize(serializer),
        }
    }
}

/// Represents a JSON-RPC error.
#[derive(thiserror::Error, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[error("The response reported error `{code}`: `{message}`. (optional data: {data:?})")]
pub struct Error<DataT = serde_json::Value> {
    /// error code
    pub code: i16,
    /// error message
    pub message: String,
    /// optional additional data
    pub data: Option<DataT>,
}

/// A JSON-RPC notification (a request without an `id`).
///
/// A `Request` object that is a `Notification` signifies the Client's lack of
/// interest in the corresponding `Response` object, and as such no `Response`
/// object needs to be returned to the client.
///
/// The Server MUST NOT reply to a `Notification`, including those that are
/// within a batch request. Notifications are not confirmable by definition,
/// since they do not have a `Response` object to be returned. As such, the
/// Client would not be aware of any errors (like e.g. "Invalid
/// params" or "Internal error").
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Notification<MethodT> {
    /// Version of the JSON-RPC protocol
    #[serde(rename = "jsonrpc")]
    pub version: Version,
    /// the method to invoke, with its parameters
    #[serde(flatten)]
    pub method: MethodT,
}

/// A JSON-RPC request
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Request<MethodT> {
    /// Version of the JSON-RPC protocol
    #[serde(rename = "jsonrpc")]
    pub version: Version,
    /// the method to invoke, with its parameters
    #[serde(flatten)]
    pub method: MethodT,
    /// An identifier established by the Client.
    ///
    /// If not included, it is assumed to be a notification.
    #[serde(default)]
    pub id: Option<Id>,
}

impl<MethodT> TryInto<Notification<MethodT>> for Request<MethodT> {
    type Error = Self;

    fn try_into(self) -> Result<Notification<MethodT>, Self::Error> {
        if self.id.is_none() {
            Ok(Notification {
                version: self.version,
                method: self.method,
            })
        } else {
            Err(self)
        }
    }
}

/// Represents a JSON-RPC 2.0 response.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Response<SuccessT, ErrorDataT = serde_json::Value> {
    /// Version of the JSON-RPC protocol
    #[serde(rename = "jsonrpc")]
    pub version: Version,
    /// Correlation id.
    ///
    /// It MUST be the same as the value of the id member in the `Request`
    /// object.
    pub id: Id,
    /// Response data.
    #[serde(flatten)]
    pub data: ResponseData<SuccessT, ErrorDataT>,
}

/// Represents JSON-RPC 2.0 success response.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponseData<SuccessT, ErrorDataT = serde_json::Value> {
    /// an error response
    Error {
        /// the error
        error: Error<ErrorDataT>,
    },
    /// a success response
    Success {
        /// the result
        result: SuccessT,
    },
}

impl<SuccessT, ErrorDataT> ResponseData<SuccessT, ErrorDataT> {
    /// Returns a [`Result`] where `Success` is mapped to `Ok` and `Error` to
    /// `Err`.
    pub fn into_result(self) -> Result<SuccessT, Error<ErrorDataT>> {
        match self {
            ResponseData::Success { result } => Ok(result),
            ResponseData::Error { error } => Err(error),
        }
    }

    /// convenience constructor for an error response
    pub fn new_error(code: i16, message: &str, data: Option<ErrorDataT>) -> Self {
        ResponseData::<SuccessT, ErrorDataT>::Error {
            error: Error {
                code,
                message: String::from(message),
                data,
            },
        }
    }
}

impl<SuccessT: Serialize, ErrorDataT, ErrorT: Into<Error<ErrorDataT>>>
    From<Result<SuccessT, ErrorT>> for ResponseData<SuccessT, ErrorDataT>
{
    fn from(result: Result<SuccessT, ErrorT>) -> Self {
        match result {
            Ok(result) => ResponseData::Success { result },
            Err(error) => ResponseData::Error {
                error: error.into(),
            },
        }
    }
}

/// Represents a JSON-RPC request/response ID.
///
/// An identifier established by the Client that MUST contain a String, Number,
/// or NULL value if included. If it is not included, it is assumed to be a
/// notification. The value SHOULD normally not be Null and Numbers SHOULD NOT
/// contain fractional parts.
///
/// If included, the Server MUST reply with the same value in the `Response`
/// object. This member is used to correlate the context between the two
/// objects.
///
/// The use of Null as a value for the `id` member in a `Request` object is
/// discouraged, because this specification uses a value of Null for `Response`s
/// with an unknown id. Also, because JSON-RPC 1.0 uses an id value of Null for
/// Notifications this could cause confusion in handling.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Id {
    /// Null ID
    Null,
    /// Numeric ID
    Number(i64),
    /// String ID
    Str(String),
}

impl<'de> serde::Deserialize<'de> for Id {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct IdVisitor;

        impl<'de> serde::de::Visitor<'de> for IdVisitor {
            type Value = Id;

            fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str("a JSON-RPC request/response ID (String, Number, or NULL)")
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Id::Null)
            }

            fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Err(serde::de::Error::custom(
                    "Numeric ID should not contain fractional parts",
                ))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Id::Number(value))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if let Ok(value) = i64::try_from(value) {
                    Ok(Id::Number(value))
                } else {
                    Err(serde::de::Error::custom(
                        "Numeric ID is too large to fit in i64",
                    ))
                }
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Id::Str(value.to_string()))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Id::Str(value))
            }
        }

        deserializer.deserialize_any(IdVisitor)
    }
}

impl serde::Serialize for Id {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Id::Null => serializer.serialize_unit(),
            Id::Number(num) => serializer.serialize_i64(*num),
            Id::Str(s) => serializer.serialize_str(s),
        }
    }
}

/// Represents JSON-RPC protocol version.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Version {
    /// Represents JSON-RPC 2.0 version.
    V2_0,
}

impl Serialize for Version {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Version::V2_0 => serializer.serialize_str("2.0"),
        }
    }
}

impl<'a> Deserialize<'a> for Version {
    fn deserialize<D>(deserializer: D) -> Result<Version, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        struct VersionVisitor;

        impl serde::de::Visitor<'_> for VersionVisitor {
            type Value = Version;

            fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str("a string specifying the version of the JSON-RPC protocol. Must be exactly \"2.0\"")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "2.0" => Ok(Version::V2_0),
                    _ => Err(serde::de::Error::custom(
                        "Invalid JSON-RPC protocol version",
                    )),
                }
            }
        }

        deserializer.deserialize_identifier(VersionVisitor)
    }
}
