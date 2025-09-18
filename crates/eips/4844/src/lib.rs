/// Information about the blob gas used in a block.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobGas {
    /// The total amount of blob gas consumed by the transactions within the
    /// block.
    pub gas_used: u64,
    /// The running total of blob gas consumed in excess of the target, prior to
    /// the block. Blocks with above-target blob gas consumption increase this
    /// value, blocks with below-target blob gas consumption decrease it
    /// (bounded at 0).
    pub excess_gas: u64,
}

// We need a custom implementation to avoid the struct being treated as an RLP
// list.
impl alloy_rlp::Decodable for BlobGas {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let blob_gas = Self {
            gas_used: u64::decode(buf)?,
            excess_gas: u64::decode(buf)?,
        };

        Ok(blob_gas)
    }
}

// We need a custom implementation to avoid the struct being treated as an RLP
// list.
impl alloy_rlp::Encodable for BlobGas {
    fn encode(&self, out: &mut dyn alloy_rlp::BufMut) {
        self.gas_used.encode(out);
        self.excess_gas.encode(out);
    }

    fn length(&self) -> usize {
        self.gas_used.length() + self.excess_gas.length()
    }
}
