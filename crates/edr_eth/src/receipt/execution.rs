mod eip2718;
mod eip658;
mod legacy;

use alloy_rlp::{Buf as _, RlpDecodable, RlpEncodable};
use revm_primitives::ChainSpec;

use super::{Execution, ExecutionReceiptBuilder, ExecutionReceiptFactory, MapReceiptLogs, Receipt};
use crate::{
    chain_spec::L1ChainSpec,
    log::ExecutionLog,
    transaction::{self, SignedTransaction as _},
    Bloom, SpecId, B256,
};

#[derive(Clone, Debug, PartialEq, Eq, RlpDecodable, RlpEncodable)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Deserialize, serde::Serialize),
    serde(rename_all = "camelCase")
)]
pub struct Legacy<LogT> {
    /// State root
    pub root: B256,
    /// Cumulative gas used in block after this transaction was executed
    #[cfg_attr(feature = "serde", serde(with = "crate::serde::u64"))]
    pub cumulative_gas_used: u64,
    /// Bloom filter of the logs generated within this transaction
    pub logs_bloom: Bloom,
    /// Logs generated within this transaction
    pub logs: Vec<LogT>,
}

#[derive(Clone, Debug, PartialEq, Eq, RlpDecodable, RlpEncodable)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Deserialize, serde::Serialize),
    serde(rename_all = "camelCase")
)]
pub struct Eip658<LogT> {
    /// Status
    #[cfg_attr(feature = "serde", serde(with = "crate::serde::bool"))]
    pub status: bool,
    /// Cumulative gas used in block after this transaction was executed
    #[cfg_attr(feature = "serde", serde(with = "crate::serde::u64"))]
    pub cumulative_gas_used: u64,
    /// Bloom filter of the logs generated within this transaction
    pub logs_bloom: Bloom,
    /// Logs generated within this transaction
    pub logs: Vec<LogT>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Deserialize, serde::Serialize),
    serde(rename_all = "camelCase")
)]
pub struct Eip2718<LogT, TypeT> {
    /// Status
    #[cfg_attr(feature = "serde", serde(with = "crate::serde::bool"))]
    pub status: bool,
    /// Cumulative gas used in block after this transaction was executed
    #[cfg_attr(feature = "serde", serde(with = "crate::serde::u64"))]
    pub cumulative_gas_used: u64,
    /// Bloom filter of the logs generated within this transaction
    pub logs_bloom: Bloom,
    /// Logs generated within this transaction
    pub logs: Vec<LogT>,
    /// Transaction type identifier
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub transaction_type: TypeT,
}

impl<LogT> From<Legacy<LogT>> for Execution<LogT> {
    fn from(value: Legacy<LogT>) -> Self {
        Execution::Legacy(value)
    }
}

impl<LogT> From<Eip658<LogT>> for Execution<LogT> {
    fn from(value: Eip658<LogT>) -> Self {
        Execution::Eip658(value)
    }
}

impl<LogT> From<Eip2718<LogT, crate::transaction::Type>> for Execution<LogT> {
    fn from(value: Eip2718<LogT, crate::transaction::Type>) -> Self {
        Execution::Eip2718(value)
    }
}

impl<LogT, NewLogT> MapReceiptLogs<LogT, NewLogT, Execution<NewLogT>> for Execution<LogT> {
    fn map_logs(self, map_fn: impl FnMut(LogT) -> NewLogT) -> Execution<NewLogT> {
        match self {
            Execution::Legacy(receipt) => Execution::Legacy(receipt.map_logs(map_fn)),
            Execution::Eip658(receipt) => Execution::Eip658(receipt.map_logs(map_fn)),
            Execution::Eip2718(receipt) => Execution::Eip2718(receipt.map_logs(map_fn)),
        }
    }
}

impl<LogT> alloy_rlp::Decodable for Execution<LogT>
where
    LogT: alloy_rlp::Decodable,
{
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        fn is_list(byte: u8) -> bool {
            byte >= 0xc0
        }

        let first = *buf.first().ok_or(alloy_rlp::Error::InputTooShort)?;
        if is_list(first) {
            // Use a temporary buffer to decode the header, avoiding the original buffer
            // from being advanced
            let first_value_header = {
                let mut temp_buf = *buf;

                let _receipt_header = alloy_rlp::Header::decode(&mut temp_buf)?;
                alloy_rlp::Header::decode(&mut temp_buf)?
            };

            // The first value of the receipt is 1 byte long, which means it's the status
            // code of an EIP-658 receipt.
            if first_value_header.payload_length == 1 {
                let receipt = Eip658::<LogT>::decode(buf)?;
                Ok(Self::Eip658(receipt))
            } else {
                let receipt = Legacy::<LogT>::decode(buf)?;
                Ok(Self::Legacy(receipt))
            }
        } else {
            // Consume the first byte
            buf.advance(1);

            let transaction_type = crate::transaction::Type::try_from(first)
                .map_err(|_error| alloy_rlp::Error::Custom("unknown receipt type"))?;

            let receipt = Eip2718::decode_with_type(buf, transaction_type)?;
            Ok(Self::Eip2718(receipt))
        }
    }
}

impl<LogT> alloy_rlp::Encodable for Execution<LogT>
where
    LogT: alloy_rlp::Encodable,
{
    fn encode(&self, out: &mut dyn alloy_rlp::BufMut) {
        match self {
            Execution::Legacy(receipt) => receipt.encode(out),
            Execution::Eip658(receipt) => receipt.encode(out),
            Execution::Eip2718(receipt) => receipt.encode(out),
        }
    }

    fn length(&self) -> usize {
        match self {
            Execution::Legacy(receipt) => receipt.length(),
            Execution::Eip658(receipt) => receipt.length(),
            Execution::Eip2718(receipt) => receipt.length(),
        }
    }
}

pub struct Builder;

impl ExecutionReceiptBuilder<L1ChainSpec> for Builder {
    type Receipt = Execution<ExecutionLog>;

    fn new_receipt_builder<StateT: revm::db::StateRef>(
        _pre_execution_state: StateT,
        _transaction: &<L1ChainSpec as ChainSpec>::Transaction,
    ) -> Result<Self, StateT::Error> {
        Ok(Self)
    }

    fn build_receipt(
        self,
        header: &crate::block::PartialHeader,
        transaction: &transaction::Signed,
        result: &revm_primitives::ExecutionResult<L1ChainSpec>,
        hardfork: SpecId,
    ) -> Self::Receipt {
        let logs = result.logs().to_vec();
        let logs_bloom = crate::log::logs_to_bloom(&logs);

        if hardfork >= SpecId::BERLIN {
            match transaction.transaction_type() {
                transaction::Type::Legacy => Execution::Eip658(Eip658 {
                    status: result.is_success(),
                    cumulative_gas_used: header.gas_used,
                    logs_bloom,
                    logs,
                }),
                transaction_type => Execution::Eip2718(Eip2718 {
                    status: result.is_success(),
                    cumulative_gas_used: header.gas_used,
                    logs_bloom,
                    logs,
                    transaction_type,
                }),
            }
        } else if hardfork >= SpecId::BYZANTIUM {
            Execution::Eip658(Eip658 {
                status: result.is_success(),
                cumulative_gas_used: header.gas_used,
                logs_bloom,
                logs,
            })
        } else {
            Execution::Legacy(Legacy {
                root: header.state_root,
                cumulative_gas_used: header.gas_used,
                logs_bloom,
                logs,
            })
        }
    }
}

impl ExecutionReceiptFactory<L1ChainSpec> for Execution<ExecutionLog> {
    type Builder = Builder;
}

impl<LogT> Receipt<LogT> for Execution<LogT> {
    type Type = crate::transaction::Type;

    fn cumulative_gas_used(&self) -> u64 {
        match self {
            Execution::Legacy(receipt) => receipt.cumulative_gas_used,
            Execution::Eip658(receipt) => receipt.cumulative_gas_used,
            Execution::Eip2718(receipt) => receipt.cumulative_gas_used,
        }
    }

    fn logs_bloom(&self) -> &Bloom {
        match self {
            Execution::Legacy(receipt) => &receipt.logs_bloom,
            Execution::Eip658(receipt) => &receipt.logs_bloom,
            Execution::Eip2718(receipt) => &receipt.logs_bloom,
        }
    }

    fn logs(&self) -> &[LogT] {
        match self {
            Execution::Legacy(receipt) => &receipt.logs,
            Execution::Eip658(receipt) => &receipt.logs,
            Execution::Eip2718(receipt) => &receipt.logs,
        }
    }

    fn root_or_status(&self) -> super::RootOrStatus<'_> {
        match self {
            Execution::Legacy(receipt) => super::RootOrStatus::Root(&receipt.root),
            Execution::Eip658(receipt) => super::RootOrStatus::Status(receipt.status),
            Execution::Eip2718(receipt) => super::RootOrStatus::Status(receipt.status),
        }
    }

    fn transaction_type(&self) -> Option<Self::Type> {
        match self {
            Execution::Legacy(_) | Execution::Eip658(_) => None,
            Execution::Eip2718(receipt) => Some(receipt.transaction_type),
        }
    }
}

// We need custom deserialization for [`Execution`] because some providers
// return the transaction type of pre-EIP-2718 receipts.
#[cfg(feature = "serde")]
impl<'deserializer, LogT> serde::Deserialize<'deserializer> for Execution<LogT>
where
    LogT: serde::Deserialize<'deserializer>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'deserializer>,
    {
        use core::marker::PhantomData;
        use std::str::FromStr as _;

        use serde::de::Visitor;

        #[derive(serde::Deserialize)]
        #[serde(field_identifier, rename_all = "camelCase")]
        enum Field {
            Type,
            Root,
            Status,
            CumulativeGasUsed,
            LogsBloom,
            Logs,
            Unknown(String),
        }

        struct ReceiptVisitor<LogT> {
            phantom: PhantomData<LogT>,
        }

        impl<'deserializer, LogT> Visitor<'deserializer> for ReceiptVisitor<LogT>
        where
            LogT: serde::Deserialize<'deserializer>,
        {
            type Value = Execution<LogT>;

            fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str("a valid receipt")
            }

            fn visit_map<MapAccessT>(
                self,
                mut map: MapAccessT,
            ) -> Result<Self::Value, MapAccessT::Error>
            where
                MapAccessT: serde::de::MapAccess<'deserializer>,
            {
                use serde::de::Error;

                use crate::U64;

                // These are `String` to support deserializing from `serde_json::Value`
                let mut transaction_type: Option<String> = None;
                let mut status_code: Option<String> = None;
                let mut state_root = None;
                let mut cumulative_gas_used: Option<U64> = None;
                let mut logs_bloom = None;
                let mut logs = None;

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
                                    "receipt cannot have both root and status",
                                ));
                            }
                            state_root = Some(map.next_value()?);
                        }
                        Field::Status => {
                            if status_code.is_some() {
                                return Err(Error::duplicate_field("status"));
                            } else if state_root.is_some() {
                                return Err(Error::custom(
                                    "receipt cannot have both root and status",
                                ));
                            }
                            status_code = Some(map.next_value()?);
                        }
                        Field::CumulativeGasUsed => {
                            if cumulative_gas_used.is_some() {
                                return Err(Error::duplicate_field("cumulativeGasUsed"));
                            }
                            cumulative_gas_used = Some(map.next_value()?);
                        }
                        Field::LogsBloom => {
                            if logs_bloom.is_some() {
                                return Err(Error::duplicate_field("logsBloom"));
                            }
                            logs_bloom = Some(map.next_value()?);
                        }
                        Field::Logs => {
                            if logs.is_some() {
                                return Err(Error::duplicate_field("logs"));
                            }
                            logs = Some(map.next_value()?);
                        }
                        Field::Unknown(field) => {
                            log::warn!("Unsupported receipt field: {field}");
                        }
                    }
                }

                let cumulative_gas_used = cumulative_gas_used
                    .ok_or_else(|| Error::missing_field("cumulativeGasUsed"))?
                    .to::<u64>();

                let logs_bloom = logs_bloom.ok_or_else(|| Error::missing_field("logsBloom"))?;
                let logs = logs.ok_or_else(|| Error::missing_field("logs"))?;

                let receipt = if let Some(state_root) = state_root {
                    Execution::Legacy(Legacy {
                        root: state_root,
                        cumulative_gas_used,
                        logs_bloom,
                        logs,
                    })
                } else if let Some(status_code) = status_code {
                    let transaction_type =
                        transaction_type.map_or(Ok(None), |transaction_type| {
                            crate::transaction::Type::from_str(&transaction_type)
                                .map(Some)
                                .map_err(|error| {
                                    Error::custom(format!("invalid transaction type: {error}"))
                                })
                        })?;

                    let status = match status_code.as_str() {
                        "0x0" => false,
                        "0x1" => true,
                        _ => return Err(Error::custom(format!("unknown status: {status_code}"))),
                    };

                    match transaction_type {
                        None | Some(crate::transaction::Type::Legacy) => {
                            Execution::Eip658(Eip658 {
                                status,
                                cumulative_gas_used,
                                logs_bloom,
                                logs,
                            })
                        }
                        Some(transaction_type) => Execution::Eip2718(Eip2718 {
                            status,
                            cumulative_gas_used,
                            logs_bloom,
                            logs,
                            transaction_type,
                        }),
                    }
                } else {
                    return Err(Error::missing_field("root or status"));
                };

                Ok(receipt)
            }
        }

        deserializer.deserialize_map(ReceiptVisitor {
            phantom: PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    use alloy_rlp::Decodable as _;

    use super::*;
    use crate::{log::ExecutionLog, Address, Bytes};

    macro_rules! impl_execution_receipt_tests {
        ($(
            $name:ident => $receipt:expr,
        )+) => {
            $(
                paste::item! {
                    #[test]
                    fn [<typed_receipt_rlp_encoding_ $name>]() {
                        let receipt = $receipt;
                        let encoded = alloy_rlp::encode(&receipt);
                        assert_eq!(Execution::<ExecutionLog>::decode(&mut encoded.as_slice()).unwrap(), receipt);
                    }

                    #[cfg(feature = "serde")]
                    #[test]
                    fn [<typed_receipt_serde_ $name>]() {
                        let receipt = $receipt;

                        let serialized = serde_json::to_string(&receipt).unwrap();
                        let deserialized: Execution<ExecutionLog> = serde_json::from_str(&serialized).unwrap();
                        assert_eq!(receipt, deserialized);

                        // This is necessary to ensure that the deser implementation doesn't expect a
                        // &str where a String can be passed.
                        let serialized = serde_json::to_value(&receipt).unwrap();
                        let deserialized: Execution<ExecutionLog> = serde_json::from_value(serialized).unwrap();

                        assert_eq!(receipt, deserialized);
                    }
                }
            )+
        };
    }

    impl_execution_receipt_tests! {
        legacy => Execution::Legacy(Legacy {
            root: B256::random(),
            cumulative_gas_used: 0xffff,
            logs_bloom: Bloom::random(),
            logs: vec![
                ExecutionLog::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::from_static(b"test"))
            ],
        }),
        eip658 => Execution::Eip658(Eip658 {
            status: true,
            cumulative_gas_used: 0xffff,
            logs_bloom: Bloom::random(),
            logs: vec![
                ExecutionLog::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::from_static(b"test"))
            ],
        }),
        eip2718 => Execution::Eip2718(Eip2718 {
            status: true,
            cumulative_gas_used: 0xffff,
            logs_bloom: Bloom::random(),
            logs: vec![
                ExecutionLog::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::from_static(b"test"))
            ],
            transaction_type: crate::transaction::Type::Eip1559,
        }),
    }
}
