mod deposit;

use std::str::FromStr as _;

use alloy_rlp::Buf as _;
pub use edr_eth::receipt::execution::{Eip2718, Eip658, Legacy};
use edr_eth::{
    log::ExecutionLog,
    receipt::{
        ExecutionReceiptBuilder, ExecutionReceiptFactory, MapReceiptLogs, Receipt, RootOrStatus,
    },
    transaction::SignedTransaction as _,
    Bloom, U8,
};
use revm::{db::StateRef, optimism::OptimismSpecId, primitives::Transaction as _};

use self::deposit::Eip2718OrDeposit;
use super::Execution;
use crate::{transaction, OptimismChainSpec};

/// Receipt for an Optimism deposit transaction with deposit nonce (since
/// Regolith) and optionally deposit receipt version (since Canyon).
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Deposit<LogT> {
    /// Status
    #[serde(with = "edr_eth::serde::bool")]
    pub status: bool,
    /// Cumulative gas used in block after this transaction was executed
    #[serde(with = "edr_eth::serde::u64")]
    pub cumulative_gas_used: u64,
    /// Bloom filter of the logs generated within this transaction
    pub logs_bloom: Bloom,
    /// Logs generated within this transaction
    pub logs: Vec<LogT>,
    /// Transaction type identifier
    #[serde(rename = "type")]
    pub transaction_type: transaction::Type,
    /// The nonce used during execution.
    #[serde(with = "edr_eth::serde::u64")]
    pub deposit_nonce: u64,
    /// The deposit receipt version.
    ///
    /// The deposit receipt version was introduced in Canyon to indicate an
    /// update to how receipt hashes should be computed when set. The state
    /// transition process ensures this is only set for post-Canyon deposit
    /// transactions.
    #[serde(
        deserialize_with = "edr_eth::serde::optional_u8::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
    pub deposit_receipt_version: Option<u8>,
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

impl<LogT> From<Eip2718<LogT, transaction::Type>> for Execution<LogT> {
    fn from(value: Eip2718<LogT, transaction::Type>) -> Self {
        Execution::Eip2718(value)
    }
}

impl<LogT> From<Deposit<LogT>> for Execution<LogT> {
    fn from(value: Deposit<LogT>) -> Self {
        Execution::Deposit(value)
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

            let transaction_type = transaction::Type::try_from(first)
                .map_err(|_error| alloy_rlp::Error::Custom("unknown receipt type"))?;

            let receipt = Eip2718OrDeposit::decode(buf)?;
            Ok(receipt.into_execution_receipt(transaction_type))
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
            Execution::Deposit(receipt) => receipt.encode(out),
        }
    }

    fn length(&self) -> usize {
        match self {
            Execution::Legacy(receipt) => receipt.length(),
            Execution::Eip658(receipt) => receipt.length(),
            Execution::Eip2718(receipt) => receipt.length(),
            Execution::Deposit(receipt) => receipt.length(),
        }
    }
}

/// Optimism execution receipt builder.
pub struct Builder {
    deposit_nonce: u64,
}

impl ExecutionReceiptBuilder<OptimismChainSpec> for Builder {
    type Receipt = Execution<ExecutionLog>;

    fn new_receipt_builder<StateT: StateRef>(
        pre_execution_state: StateT,
        transaction: &transaction::Signed,
    ) -> Result<Self, StateT::Error> {
        let deposit_nonce = pre_execution_state
            .basic(*transaction.caller())?
            .map_or(0, |account| account.nonce);

        Ok(Self { deposit_nonce })
    }

    fn build_receipt(
        self,
        header: &edr_eth::block::PartialHeader,
        transaction: &<OptimismChainSpec as revm::primitives::ChainSpec>::Transaction,
        result: &revm::primitives::ExecutionResult<OptimismChainSpec>,
        hardfork: <OptimismChainSpec as revm::primitives::ChainSpec>::Hardfork,
    ) -> Self::Receipt {
        let logs = result.logs().to_vec();
        let logs_bloom = edr_eth::log::logs_to_bloom(&logs);

        if hardfork >= OptimismSpecId::BERLIN {
            match transaction.transaction_type() {
                transaction::Type::Legacy => Execution::Eip658(Eip658 {
                    status: result.is_success(),
                    cumulative_gas_used: header.gas_used,
                    logs_bloom,
                    logs,
                }),
                transaction::Type::Deposit => Execution::Deposit(Deposit {
                    status: result.is_success(),
                    cumulative_gas_used: header.gas_used,
                    logs_bloom,
                    logs,
                    transaction_type: transaction::Type::Deposit,
                    deposit_nonce: self.deposit_nonce,
                    deposit_receipt_version: if hardfork >= OptimismSpecId::CANYON {
                        Some(1)
                    } else {
                        None
                    },
                }),
                transaction_type => Execution::Eip2718(Eip2718 {
                    status: result.is_success(),
                    cumulative_gas_used: header.gas_used,
                    logs_bloom,
                    logs,
                    transaction_type,
                }),
            }
        } else if hardfork >= OptimismSpecId::BYZANTIUM {
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

impl ExecutionReceiptFactory<OptimismChainSpec> for Execution<ExecutionLog> {
    type Builder = Builder;
}

impl<LogT, NewLogT> MapReceiptLogs<LogT, NewLogT, Execution<NewLogT>> for Execution<LogT> {
    fn map_logs(self, map_fn: impl FnMut(LogT) -> NewLogT) -> Execution<NewLogT> {
        match self {
            Execution::Legacy(receipt) => Execution::Legacy(receipt.map_logs(map_fn)),
            Execution::Eip658(receipt) => Execution::Eip658(receipt.map_logs(map_fn)),
            Execution::Eip2718(receipt) => Execution::Eip2718(receipt.map_logs(map_fn)),
            Execution::Deposit(receipt) => Execution::Deposit(Deposit {
                status: receipt.status,
                cumulative_gas_used: receipt.cumulative_gas_used,
                logs_bloom: receipt.logs_bloom,
                logs: receipt.logs.into_iter().map(map_fn).collect(),
                transaction_type: receipt.transaction_type,
                deposit_nonce: receipt.deposit_nonce,
                deposit_receipt_version: receipt.deposit_receipt_version,
            }),
        }
    }
}

impl<LogT> Receipt<LogT> for Execution<LogT> {
    type Type = transaction::Type;

    fn cumulative_gas_used(&self) -> u64 {
        match self {
            Execution::Legacy(receipt) => receipt.cumulative_gas_used,
            Execution::Eip658(receipt) => receipt.cumulative_gas_used,
            Execution::Eip2718(receipt) => receipt.cumulative_gas_used,
            Execution::Deposit(receipt) => receipt.cumulative_gas_used,
        }
    }

    fn logs_bloom(&self) -> &Bloom {
        match self {
            Execution::Legacy(receipt) => &receipt.logs_bloom,
            Execution::Eip658(receipt) => &receipt.logs_bloom,
            Execution::Eip2718(receipt) => &receipt.logs_bloom,
            Execution::Deposit(receipt) => &receipt.logs_bloom,
        }
    }

    fn logs(&self) -> &[LogT] {
        match self {
            Execution::Legacy(receipt) => &receipt.logs,
            Execution::Eip658(receipt) => &receipt.logs,
            Execution::Eip2718(receipt) => &receipt.logs,
            Execution::Deposit(receipt) => &receipt.logs,
        }
    }

    fn root_or_status(&self) -> edr_eth::receipt::RootOrStatus<'_> {
        match self {
            Execution::Legacy(receipt) => RootOrStatus::Root(&receipt.root),
            Execution::Eip658(receipt) => RootOrStatus::Status(receipt.status),
            Execution::Eip2718(receipt) => RootOrStatus::Status(receipt.status),
            Execution::Deposit(receipt) => RootOrStatus::Status(receipt.status),
        }
    }

    fn transaction_type(&self) -> Option<Self::Type> {
        match self {
            Execution::Legacy(_) | Execution::Eip658(_) => None,
            Execution::Eip2718(receipt) => Some(receipt.transaction_type),
            Execution::Deposit(receipt) => Some(receipt.transaction_type),
        }
    }
}

// We need custom deserialization for [`Execution`] because some providers
// return the transaction type of pre-EIP-2718 receipts.
impl<'deserializer, LogT> serde::Deserialize<'deserializer> for Execution<LogT>
where
    LogT: serde::Deserialize<'deserializer>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'deserializer>,
    {
        use core::marker::PhantomData;

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
            DepositNonce,
            DepositReceiptVersion,
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
                use edr_eth::U64;
                use serde::de::Error;

                // These are `String` to support deserializing from `serde_json::Value`
                let mut transaction_type: Option<String> = None;
                let mut status_code: Option<String> = None;
                let mut state_root = None;
                let mut cumulative_gas_used: Option<U64> = None;
                let mut logs_bloom = None;
                let mut logs = None;
                let mut deposit_nonce: Option<U64> = None;
                let mut deposit_receipt_version: Option<U8> = None;

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
                        Field::DepositNonce => {
                            deposit_nonce = Some(map.next_value()?);
                        }
                        Field::DepositReceiptVersion => {
                            deposit_receipt_version = Some(map.next_value()?);
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
                            transaction::Type::from_str(&transaction_type)
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
                        None | Some(transaction::Type::Legacy) => Execution::Eip658(Eip658 {
                            status,
                            cumulative_gas_used,
                            logs_bloom,
                            logs,
                        }),
                        Some(transaction_type) => {
                            if let Some(deposit_nonce) = deposit_nonce {
                                Execution::Deposit(Deposit {
                                    status,
                                    cumulative_gas_used,
                                    logs_bloom,
                                    logs,
                                    transaction_type,
                                    deposit_nonce: deposit_nonce.to(),
                                    deposit_receipt_version: deposit_receipt_version
                                        .map(|version| version.to()),
                                })
                            } else {
                                Execution::Eip2718(Eip2718 {
                                    status,
                                    cumulative_gas_used,
                                    logs_bloom,
                                    logs,
                                    transaction_type,
                                })
                            }
                        }
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
    use edr_eth::{log::ExecutionLog, Address, Bytes, B256};

    use super::*;

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
        deposit => Execution::Deposit(Deposit {
            status: true,
            cumulative_gas_used: 0xffff,
            logs_bloom: Bloom::random(),
            logs: vec![
                ExecutionLog::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::from_static(b"test"))
            ],
            transaction_type: crate::transaction::Type::Eip1559,
            deposit_nonce: 0x1234,
            deposit_receipt_version: Some(0x01),
        }),
    }
}
