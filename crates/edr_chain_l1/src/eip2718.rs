//! Types related to EIP-2718.

use alloy_rlp::Buf as _;
use edr_primitives::Bloom;
use edr_receipt::{ExecutionReceipt, MapReceiptLogs, RootOrStatus};
use edr_transaction::TransactionType;

use crate::L1TransactionType;

/// An compile-time typed EIP-2718 envelope for L1 Ethereum.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TypedEnvelope<DataT> {
    /// Legacy transaction.
    Legacy(DataT),
    /// EIP-2930 transaction.
    Eip2930(DataT),
    /// EIP-1559 transaction.
    Eip1559(DataT),
    /// EIP-4844 transaction.
    Eip4844(DataT),
    /// EIP-7702 transaction.
    Eip7702(DataT),
}

impl<DataT> TypedEnvelope<DataT> {
    /// Constructs a typed envelope around the given data.
    pub fn new(data: DataT, transaction_type: L1TransactionType) -> Self {
        match transaction_type {
            L1TransactionType::Legacy => Self::Legacy(data),
            L1TransactionType::Eip2930 => Self::Eip2930(data),
            L1TransactionType::Eip1559 => Self::Eip1559(data),
            L1TransactionType::Eip4844 => Self::Eip4844(data),
            L1TransactionType::Eip7702 => Self::Eip7702(data),
        }
    }

    /// Returns a reference to the data inside the envelope.
    pub fn data(&self) -> &DataT {
        match self {
            TypedEnvelope::Legacy(data)
            | TypedEnvelope::Eip2930(data)
            | TypedEnvelope::Eip1559(data)
            | TypedEnvelope::Eip4844(data)
            | TypedEnvelope::Eip7702(data) => data,
        }
    }

    /// Maps the data inside the envelope to a new type.
    pub fn map<NewDataT, F>(self, f: F) -> TypedEnvelope<NewDataT>
    where
        F: FnOnce(DataT) -> NewDataT,
    {
        match self {
            TypedEnvelope::Legacy(data) => TypedEnvelope::Legacy(f(data)),
            TypedEnvelope::Eip2930(data) => TypedEnvelope::Eip2930(f(data)),
            TypedEnvelope::Eip1559(data) => TypedEnvelope::Eip1559(f(data)),
            TypedEnvelope::Eip4844(data) => TypedEnvelope::Eip4844(f(data)),
            TypedEnvelope::Eip7702(data) => TypedEnvelope::Eip7702(f(data)),
        }
    }
}

impl<DataT> TransactionType for TypedEnvelope<DataT> {
    type Type = L1TransactionType;

    fn transaction_type(&self) -> Self::Type {
        match self {
            TypedEnvelope::Legacy(_) => L1TransactionType::Legacy,
            TypedEnvelope::Eip2930(_) => L1TransactionType::Eip2930,
            TypedEnvelope::Eip1559(_) => L1TransactionType::Eip1559,
            TypedEnvelope::Eip4844(_) => L1TransactionType::Eip4844,
            TypedEnvelope::Eip7702(_) => L1TransactionType::Eip7702,
        }
    }
}

impl<DataT> alloy_rlp::Decodable for TypedEnvelope<DataT>
where
    DataT: alloy_rlp::Decodable,
{
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        fn is_list(byte: u8) -> bool {
            byte >= 0xc0
        }

        let first = *buf.first().ok_or(alloy_rlp::Error::InputTooShort)?;
        let transaction_type = if is_list(first) {
            L1TransactionType::Legacy
        } else {
            // Consume the first byte
            buf.advance(1);

            crate::L1TransactionType::try_from(first)
                .map_err(|_error| alloy_rlp::Error::Custom("unknown receipt type"))?
        };

        let data = DataT::decode(buf)?;
        Ok(TypedEnvelope::new(data, transaction_type))
    }
}

impl<DataT> alloy_rlp::Encodable for TypedEnvelope<DataT>
where
    DataT: alloy_rlp::Encodable,
{
    fn encode(&self, out: &mut dyn alloy_rlp::BufMut) {
        let transaction_type: u8 = self.transaction_type().into();
        if transaction_type > 0 {
            out.put_u8(transaction_type);
        }

        self.data().encode(out);
    }

    fn length(&self) -> usize {
        let type_length = usize::from(u8::from(self.transaction_type()) > 0u8);
        type_length + self.data().length()
    }
}

impl<DataT: ExecutionReceipt> ExecutionReceipt for TypedEnvelope<DataT> {
    type Log = DataT::Log;

    fn cumulative_gas_used(&self) -> u64 {
        self.data().cumulative_gas_used()
    }

    fn logs_bloom(&self) -> &Bloom {
        self.data().logs_bloom()
    }

    fn transaction_logs(&self) -> &[Self::Log] {
        self.data().transaction_logs()
    }

    fn root_or_status(&self) -> RootOrStatus<'_> {
        self.data().root_or_status()
    }
}

impl<OldDataT: MapReceiptLogs<OldLogT, NewLogT, NewDataT>, OldLogT, NewLogT, NewDataT>
    MapReceiptLogs<OldLogT, NewLogT, TypedEnvelope<NewDataT>> for TypedEnvelope<OldDataT>
{
    fn map_logs(self, map_fn: impl FnMut(OldLogT) -> NewLogT) -> TypedEnvelope<NewDataT> {
        self.map(|data| data.map_logs(map_fn))
    }
}

#[cfg(test)]
mod tests {
    use alloy_rlp::Decodable as _;
    use edr_primitives::{Address, Bytes, B256};
    use edr_receipt::{log::ExecutionLog, Execution};

    use super::*;

    macro_rules! impl_execution_receipt_tests {
        ($(
            $name:ident: $execution_log_ty:ty => $receipt:expr,
        )+) => {
            $(
                paste::item! {
                    #[test]
                    fn [<typed_receipt_rlp_encoding_ $name>]() -> anyhow::Result<()> {
                        let receipt = $receipt;

                        let encoded = alloy_rlp::encode(&receipt);
                        let decoded = TypedEnvelope::<$execution_log_ty>::decode(&mut encoded.as_slice())?;
                        assert_eq!(decoded, receipt);

                        Ok(())
                    }
                }
            )+
        };
    }

    impl_execution_receipt_tests! {
        legacy_legacy: Execution<ExecutionLog> => TypedEnvelope::Legacy(Execution::Legacy(edr_receipt::execution::Legacy {
            root: B256::random(),
            cumulative_gas_used: 0xffff,
            logs_bloom: Bloom::random(),
            logs: vec![
                ExecutionLog::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::from_static(b"test"))
            ],
        })),
        eip658_legacy: Execution<ExecutionLog> => TypedEnvelope::Legacy(Execution::Eip658(edr_receipt::execution::Eip658 {
            status: true,
            cumulative_gas_used: 0xffff,
            logs_bloom: Bloom::random(),
            logs: vec![
                ExecutionLog::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::from_static(b"test"))
            ],
        })),
        eip658_eip2930: Execution<ExecutionLog> => TypedEnvelope::Eip2930(Execution::Eip658(edr_receipt::execution::Eip658 {
            status: true,
            cumulative_gas_used: 0xffff,
            logs_bloom: Bloom::random(),
            logs: vec![
                ExecutionLog::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::from_static(b"test"))
            ],
        })),
        eip658_eip1559: Execution<ExecutionLog> => TypedEnvelope::Eip2930(Execution::Eip658(edr_receipt::execution::Eip658 {
            status: true,
            cumulative_gas_used: 0xffff,
            logs_bloom: Bloom::random(),
            logs: vec![
                ExecutionLog::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::from_static(b"test"))
            ],
        })),
        eip658_eip4844: Execution<ExecutionLog> => TypedEnvelope::Eip4844(Execution::Eip658(edr_receipt::execution::Eip658 {
            status: true,
            cumulative_gas_used: 0xffff,
            logs_bloom: Bloom::random(),
            logs: vec![
                ExecutionLog::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::from_static(b"test"))
            ],
        })),
        eip658_eip7702: Execution<ExecutionLog> => TypedEnvelope::Eip7702(Execution::Eip658(edr_receipt::execution::Eip658 {
            status: true,
            cumulative_gas_used: 0xffff,
            logs_bloom: Bloom::random(),
            logs: vec![
                ExecutionLog::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::from_static(b"test"))
            ],
        })),
        legacy: edr_receipt::execution::Eip658<ExecutionLog> => TypedEnvelope::Legacy(edr_receipt::execution::Eip658 {
            status: true,
            cumulative_gas_used: 0xffff,
            logs_bloom: Bloom::random(),
            logs: vec![
                ExecutionLog::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::from_static(b"test"))
            ],
        }),
        eip2930: edr_receipt::execution::Eip658<ExecutionLog> => TypedEnvelope::Eip2930(edr_receipt::execution::Eip658 {
            status: true,
            cumulative_gas_used: 0xffff,
            logs_bloom: Bloom::random(),
            logs: vec![
                ExecutionLog::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::from_static(b"test"))
            ],
        }),
        eip1559: edr_receipt::execution::Eip658<ExecutionLog> => TypedEnvelope::Eip2930(edr_receipt::execution::Eip658 {
            status: true,
            cumulative_gas_used: 0xffff,
            logs_bloom: Bloom::random(),
            logs: vec![
                ExecutionLog::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::from_static(b"test"))
            ],
        }),
        eip4844: edr_receipt::execution::Eip658<ExecutionLog> => TypedEnvelope::Eip4844(edr_receipt::execution::Eip658 {
            status: true,
            cumulative_gas_used: 0xffff,
            logs_bloom: Bloom::random(),
            logs: vec![
                ExecutionLog::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::from_static(b"test"))
            ],
        }),
        eip7702: edr_receipt::execution::Eip658<ExecutionLog> => TypedEnvelope::Eip7702(edr_receipt::execution::Eip658 {
            status: true,
            cumulative_gas_used: 0xffff,
            logs_bloom: Bloom::random(),
            logs: vec![
                ExecutionLog::new_unchecked(Address::random(), vec![B256::random(), B256::random()], Bytes::new()),
                ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::from_static(b"test"))
            ],
        }),
    }
}
