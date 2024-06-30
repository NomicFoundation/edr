// Part of this code was adapted from foundry and is distributed under their
// licenss:
// - https://github.com/foundry-rs/foundry/blob/01b16238ff87dc7ca8ee3f5f13e389888c2a2ee4/LICENSE-APACHE
// - https://github.com/foundry-rs/foundry/blob/01b16238ff87dc7ca8ee3f5f13e389888c2a2ee4/LICENSE-MIT
// For the original context see: https://github.com/foundry-rs/foundry/blob/01b16238ff87dc7ca8ee3f5f13e389888c2a2ee4/anvil/core/src/eth/receipt.rs

#![allow(missing_docs)]

mod block;
mod transaction;

use alloy_rlp::{Buf, BufMut, Decodable, Encodable};

pub use self::{block::BlockReceipt, transaction::TransactionReceipt};
use crate::{Bloom, B256};

/// Typed receipt that's generated after execution of a transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Deserialize, serde::Serialize),
    serde(rename_all = "camelCase")
)]
pub struct TypedReceipt<LogT> {
    /// Cumulative gas used in block after this transaction was executed
    #[cfg_attr(feature = "serde", serde(with = "crate::serde::u64"))]
    pub cumulative_gas_used: u64,
    /// Bloom filter of the logs generated within this transaction
    pub logs_bloom: Bloom,
    /// Logs generated within this transaction
    pub logs: Vec<LogT>,
    /// The typed receipt data.
    /// - `root` field (before Byzantium) or `status` field (after Byzantium)
    /// - `type` field after Berlin
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub data: TypedReceiptData,
}

impl<LogT> TypedReceipt<LogT> {
    /// Returns the status code of the receipt, if any.
    pub fn status_code(&self) -> Option<u8> {
        match &self.data {
            TypedReceiptData::PreEip658Legacy { .. } => None,
            TypedReceiptData::PostEip658Legacy { status }
            | TypedReceiptData::Eip2930 { status }
            | TypedReceiptData::Eip1559 { status }
            | TypedReceiptData::Eip4844 { status } => Some(*status),
        }
    }

    /// Returns the state root of the receipt, if any.
    pub fn state_root(&self) -> Option<&B256> {
        match &self.data {
            TypedReceiptData::PreEip658Legacy { state_root } => Some(state_root),
            _ => None,
        }
    }

    /// Returns the transaction type of the receipt.
    pub fn transaction_type(&self) -> crate::transaction::Type {
        self.data.transaction_type()
    }
}

impl<LogT> TypedReceipt<LogT>
where
    LogT: Encodable,
{
    fn rlp_payload_length(&self) -> usize {
        let data_length = match &self.data {
            TypedReceiptData::PreEip658Legacy { state_root } => state_root.length(),
            TypedReceiptData::PostEip658Legacy { .. }
            | TypedReceiptData::Eip2930 { .. }
            | TypedReceiptData::Eip1559 { .. }
            | TypedReceiptData::Eip4844 { .. } => 1,
        };

        data_length
            + self.cumulative_gas_used.length()
            + self.logs_bloom.length()
            + self.logs.length()
    }
}

/// Data of a typed receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypedReceiptData {
    PreEip658Legacy { state_root: B256 },
    PostEip658Legacy { status: u8 },
    Eip2930 { status: u8 },
    Eip1559 { status: u8 },
    Eip4844 { status: u8 },
}

impl TypedReceiptData {
    /// Returns the transaction type of the receipt.
    pub fn transaction_type(&self) -> crate::transaction::Type {
        match &self {
            TypedReceiptData::PreEip658Legacy { .. }
            | TypedReceiptData::PostEip658Legacy { .. } => crate::transaction::Type::Legacy,
            TypedReceiptData::Eip2930 { .. } => crate::transaction::Type::Eip2930,
            TypedReceiptData::Eip1559 { .. } => crate::transaction::Type::Eip1559,
            TypedReceiptData::Eip4844 { .. } => crate::transaction::Type::Eip4844,
        }
    }
}

#[cfg(feature = "serde")]
impl<'deserializer> serde::Deserialize<'deserializer> for TypedReceiptData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'deserializer>,
    {
        use serde::de::Visitor;

        #[derive(serde::Deserialize)]
        #[serde(field_identifier, rename_all = "camelCase")]
        enum Field {
            Type,
            Root,
            Status,
            Unknown(String),
        }

        struct TypedReceiptDataVisitor;

        impl<'deserializer> Visitor<'deserializer> for TypedReceiptDataVisitor {
            type Value = TypedReceiptData;

            fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str("valid receipt data")
            }

            fn visit_map<MapAccessT>(
                self,
                mut map: MapAccessT,
            ) -> Result<Self::Value, MapAccessT::Error>
            where
                MapAccessT: serde::de::MapAccess<'deserializer>,
            {
                use serde::de::Error;

                // These are `String` to support deserializing from `serde_json::Value`
                let mut transaction_type: Option<String> = None;
                let mut status_code: Option<String> = None;
                let mut state_root = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Type => {
                            if transaction_type.is_some() {
                                return Err(Error::duplicate_field("type"));
                            }
                            transaction_type = Some(map.next_value()?);
                        }
                        Field::Root => {
                            if state_root.is_some() {
                                return Err(Error::duplicate_field("root"));
                            } else if status_code.is_some() {
                                return Err(Error::custom(
                                    "root and status cannot be present together",
                                ));
                            }
                            state_root = Some(map.next_value()?);
                        }
                        Field::Status => {
                            if status_code.is_some() {
                                return Err(Error::duplicate_field("status"));
                            } else if state_root.is_some() {
                                return Err(Error::custom(
                                    "root and status cannot be present together",
                                ));
                            }
                            status_code = Some(map.next_value()?);
                        }
                        Field::Unknown(field) => {
                            log::warn!("Unsupported receipt field: {field}");
                        }
                    }
                }

                let data = if let Some(state_root) = state_root {
                    TypedReceiptData::PreEip658Legacy { state_root }
                } else if let Some(status_code) = status_code {
                    let status = match status_code.as_str() {
                        "0x0" => 0u8,
                        "0x1" => 1u8,
                        _ => return Err(Error::custom(format!("unknown status: {status_code}"))),
                    };

                    if let Some(transaction_type) = transaction_type {
                        match transaction_type.as_str() {
                            "0x0" => TypedReceiptData::PostEip658Legacy { status },
                            "0x1" => TypedReceiptData::Eip2930 { status },
                            "0x2" => TypedReceiptData::Eip1559 { status },
                            "0x3" => TypedReceiptData::Eip4844 { status },
                            _ => {
                                log::warn!("Unsupported receipt type: {transaction_type}. Reverting to post-EIP 155 legacy receipt");
                                TypedReceiptData::PostEip658Legacy { status }
                            }
                        }
                    } else {
                        TypedReceiptData::PostEip658Legacy { status }
                    }
                } else {
                    return Err(Error::missing_field("root or status"));
                };

                Ok(data)
            }
        }

        deserializer.deserialize_map(TypedReceiptDataVisitor)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for TypedReceiptData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        use crate::U8;

        // Pre-EIP-2178 receipts have no type field.
        // <https://eips.ethereum.org/EIPS/eip-2718>
        let tx_type = self.transaction_type();
        let should_serialize_type = self.transaction_type() >= crate::transaction::Type::Legacy;
        let num_fields = if should_serialize_type { 2 } else { 1 };

        let mut state = serializer.serialize_struct("TypedReceipt", num_fields)?;

        if should_serialize_type {
            state.serialize_field("type", &U8::from(u8::from(tx_type)))?;
        }

        match &self {
            TypedReceiptData::PreEip658Legacy { state_root } => {
                state.serialize_field("root", state_root)?;
            }
            TypedReceiptData::PostEip658Legacy { status }
            | TypedReceiptData::Eip2930 { status }
            | TypedReceiptData::Eip1559 { status }
            | TypedReceiptData::Eip4844 { status } => {
                state.serialize_field("status", &format!("0x{status}"))?;
            }
        }

        state.end()
    }
}

impl<LogT> Decodable for TypedReceipt<LogT>
where
    LogT: Decodable,
{
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        fn decode_inner<LogT>(
            buf: &mut &[u8],
            id: Option<u8>,
        ) -> Result<TypedReceipt<LogT>, alloy_rlp::Error>
        where
            LogT: Decodable,
        {
            fn normalize_status(status: u8) -> u8 {
                u8::from(status == 1)
            }

            let alloy_rlp::Header {
                list,
                payload_length,
            } = alloy_rlp::Header::decode(buf)?;

            if !list {
                return Err(alloy_rlp::Error::UnexpectedString);
            }

            let started_len = buf.len();
            if started_len < payload_length {
                return Err(alloy_rlp::Error::InputTooShort);
            }

            let data = match id {
                None | Some(0) => {
                    // Use a temporary buffer to decode the header, avoiding the original buffer
                    // from being advanced
                    let header = {
                        let mut temp_buf = *buf;
                        alloy_rlp::Header::decode(&mut temp_buf)?
                    };

                    if header.payload_length == 1 {
                        let status = u8::decode(buf)?;
                        TypedReceiptData::PostEip658Legacy {
                            status: normalize_status(status),
                        }
                    } else {
                        TypedReceiptData::PreEip658Legacy {
                            state_root: B256::decode(buf)?,
                        }
                    }
                }
                Some(1) => TypedReceiptData::Eip2930 {
                    status: normalize_status(u8::decode(buf)?),
                },
                Some(2) => TypedReceiptData::Eip1559 {
                    status: normalize_status(u8::decode(buf)?),
                },
                _ => return Err(alloy_rlp::Error::Custom("Unknown receipt type")),
            };

            let receipt = TypedReceipt {
                cumulative_gas_used: u64::decode(buf)?,
                logs_bloom: Bloom::decode(buf)?,
                logs: Vec::<LogT>::decode(buf)?,
                data,
            };

            let consumed = started_len - buf.len();
            if consumed != payload_length {
                return Err(alloy_rlp::Error::ListLengthMismatch {
                    expected: payload_length,
                    got: consumed,
                });
            }

            Ok(receipt)
        }

        fn is_list(byte: u8) -> bool {
            byte >= 0xc0
        }

        let first = *buf.first().ok_or(alloy_rlp::Error::InputTooShort)?;
        let id = if is_list(first) {
            None
        } else {
            // Consume the first byte
            buf.advance(1);

            match first {
                0x01 => Some(1u8),
                0x02 => Some(2u8),
                0x03 => Some(3u8),
                _ => return Err(alloy_rlp::Error::Custom("unknown receipt type")),
            }
        };

        decode_inner(buf, id)
    }
}

impl<LogT> Encodable for TypedReceipt<LogT>
where
    LogT: Encodable,
{
    fn encode(&self, out: &mut dyn BufMut) {
        let id = match &self.data {
            TypedReceiptData::PreEip658Legacy { .. }
            | TypedReceiptData::PostEip658Legacy { .. } => None,
            TypedReceiptData::Eip2930 { .. } => Some(1u8),
            TypedReceiptData::Eip1559 { .. } => Some(2u8),
            TypedReceiptData::Eip4844 { .. } => Some(3u8),
        };

        if let Some(id) = id {
            out.put_u8(id);
        }

        alloy_rlp::Header {
            list: true,
            payload_length: self.rlp_payload_length(),
        }
        .encode(out);

        match &self.data {
            TypedReceiptData::PreEip658Legacy { state_root } => {
                state_root.encode(out);
            }
            TypedReceiptData::PostEip658Legacy { status }
            | TypedReceiptData::Eip2930 { status }
            | TypedReceiptData::Eip1559 { status }
            | TypedReceiptData::Eip4844 { status } => {
                if *status == 0 {
                    out.put_u8(alloy_rlp::EMPTY_STRING_CODE);
                } else {
                    out.put_u8(1);
                }
            }
        }

        self.cumulative_gas_used.encode(out);
        self.logs_bloom.encode(out);
        self.logs.encode(out);
    }

    fn length(&self) -> usize {
        // Post-EIP-2930 receipts have an id byte
        let index_length = match self.data {
            TypedReceiptData::PreEip658Legacy { .. }
            | TypedReceiptData::PostEip658Legacy { .. } => 0,
            TypedReceiptData::Eip2930 { .. }
            | TypedReceiptData::Eip1559 { .. }
            | TypedReceiptData::Eip4844 { .. } => 1,
        };

        let payload_length = self.rlp_payload_length();
        index_length + payload_length + alloy_rlp::length_of_length(payload_length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{log::Log, Address, Bytes};

    macro_rules! impl_typed_receipt_tests {
        ($(
            $name:ident => $receipt_data:expr,
        )+) => {
            $(
                paste::item! {
                    fn [<typed_receipt_dummy_ $name>]() -> TypedReceipt<Log> {
                        TypedReceipt {
                            cumulative_gas_used: 0xffff,
                            logs_bloom: Bloom::random(),
                            logs: vec![
                                Log::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                                Log::new_unchecked(Address::random(), Vec::new(), Bytes::from_static(b"test"))
                            ],
                            data: $receipt_data,
                        }
                    }

                    #[test]
                    fn [<typed_receipt_rlp_encoding_ $name>]() {
                        let receipt = [<typed_receipt_dummy_ $name>]();
                        let encoded = alloy_rlp::encode(&receipt);
                        assert_eq!(TypedReceipt::<Log>::decode(&mut encoded.as_slice()).unwrap(), receipt);
                    }

                    #[cfg(feature = "serde")]
                    #[test]
                    fn [<typed_receipt_serde_ $name>]() {
                        let receipt = [<typed_receipt_dummy_ $name>]();

                        let serialized = serde_json::to_string(&receipt).unwrap();
                        let deserialized: TypedReceipt<Log> = serde_json::from_str(&serialized).unwrap();
                        assert_eq!(receipt, deserialized);

                        // This is necessary to ensure that the deser implementation doesn't expect a
                        // &str where a String can be passed.
                        let serialized = serde_json::to_value(&receipt).unwrap();
                        let deserialized: TypedReceipt<Log> = serde_json::from_value(serialized).unwrap();

                        assert_eq!(receipt, deserialized);
                    }
                }
            )+
        };
    }

    impl_typed_receipt_tests! {
        pre_eip658 => TypedReceiptData::PreEip658Legacy {
            state_root: B256::random(),
        },
        post_eip658 => TypedReceiptData::PostEip658Legacy { status: 1 },
        eip2930 => TypedReceiptData::Eip2930 { status: 1 },
        eip1559 => TypedReceiptData::Eip1559 { status: 0 },
    }
}
