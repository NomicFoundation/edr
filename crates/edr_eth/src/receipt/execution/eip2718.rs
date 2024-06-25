use alloy_rlp::{Decodable as _, RlpEncodable};
use revm_primitives::alloy_primitives::Bloom;

use super::{Eip2718, Eip658, MapReceiptLogs};

#[derive(RlpEncodable)]
struct Encodable<'receipt, LogT> {
    status: bool,
    cumulative_gas_used: u64,
    logs_bloom: &'receipt Bloom,
    logs: &'receipt Vec<LogT>,
}

impl<LogT, TypeT> Eip2718<LogT, TypeT>
where
    LogT: alloy_rlp::Decodable,
{
    /// Decodes an EIP-658 receipt with the given transaction type.
    pub fn decode_with_type(
        buf: &mut &[u8],
        transaction_type: TypeT,
    ) -> Result<Self, alloy_rlp::Error> {
        let receipt = Eip658::<LogT>::decode(buf)?;

        Ok(Self {
            status: receipt.status,
            cumulative_gas_used: receipt.cumulative_gas_used,
            logs_bloom: receipt.logs_bloom,
            logs: receipt.logs,
            transaction_type,
        })
    }
}

impl<LogT, TypeT> alloy_rlp::Encodable for Eip2718<LogT, TypeT>
where
    LogT: alloy_rlp::Encodable,
    TypeT: Copy + Into<u8>,
{
    fn encode(&self, out: &mut dyn alloy_rlp::BufMut) {
        out.put_u8(self.transaction_type.into());

        Encodable::from(self).encode(out);
    }

    fn length(&self) -> usize {
        1 + Encodable::from(self).length()
    }
}

impl<'encodable, LogT, TypeT> From<&'encodable Eip2718<LogT, TypeT>>
    for Encodable<'encodable, LogT>
{
    fn from(receipt: &'encodable Eip2718<LogT, TypeT>) -> Self {
        Self {
            status: receipt.status,
            cumulative_gas_used: receipt.cumulative_gas_used,
            logs_bloom: &receipt.logs_bloom,
            logs: &receipt.logs,
        }
    }
}

impl<LogT, NewLogT, TypeT> MapReceiptLogs<LogT, NewLogT, Eip2718<NewLogT, TypeT>>
    for Eip2718<LogT, TypeT>
{
    fn map_logs(self, map_fn: impl FnMut(LogT) -> NewLogT) -> Eip2718<NewLogT, TypeT> {
        Eip2718 {
            status: self.status,
            cumulative_gas_used: self.cumulative_gas_used,
            logs_bloom: self.logs_bloom,
            logs: self.logs.into_iter().map(map_fn).collect(),
            transaction_type: self.transaction_type,
        }
    }
}
