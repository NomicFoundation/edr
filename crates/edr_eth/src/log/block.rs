use std::ops::Deref;

use alloy_rlp::BufMut;

use super::receipt::ReceiptLog;
use crate::B256;

/// A log that's returned by a block query.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(untagged))]
pub enum BlockLog {
    /// A full log.
    Full(FullBlockLog),
    /// A partial log, which can only occur for pending blocks.
    Partial(ReceiptLog),
}

/// A type representing a fully specified block log.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct FullBlockLog {
    /// Receipt log
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub inner: ReceiptLog,
    /// block hash
    // https://github.com/NomicFoundation/hardhat/blob/7d25b1b5a7bfbd7e7fabbf540b0f32186cba2b11/packages/hardhat-core/src/internal/hardhat-network/provider/output.ts#L120
    pub block_hash: B256,
    /// block number
    #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
    pub block_number: u64,
    /// Index of the log within the block
    #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
    pub log_index: u64,
    /// Index of the transaction within the block
    #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
    pub transaction_index: u64,
}

impl Deref for FullBlockLog {
    type Target = ReceiptLog;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl alloy_rlp::Encodable for BlockLog {
    fn encode(&self, out: &mut dyn BufMut) {
        match self {
            BlockLog::Partial(log) => log.encode(out),
            BlockLog::Full(log) => log.encode(out),
        }
    }

    fn length(&self) -> usize {
        match self {
            BlockLog::Partial(log) => log.length(),
            BlockLog::Full(log) => log.length(),
        }
    }
}

impl alloy_rlp::Encodable for FullBlockLog {
    fn encode(&self, out: &mut dyn BufMut) {
        self.inner.encode(out);
    }

    fn length(&self) -> usize {
        self.inner.length()
    }
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::{log::ExecutionLog, Address, Bytes};

    #[test]
    fn test_block_log_full_serde() -> anyhow::Result<()> {
        let log = BlockLog::Full(FullBlockLog {
            inner: ReceiptLog {
                inner: ExecutionLog::new_unchecked(
                    Address::from_str("0000000000000000000000000000000000000011")?,
                    vec![
                        B256::from_str(
                            "000000000000000000000000000000000000000000000000000000000000dead",
                        )?,
                        B256::from_str(
                            "000000000000000000000000000000000000000000000000000000000000beef",
                        )?,
                    ],
                    Bytes::from(hex::decode("0100ff")?),
                ),
                transaction_hash: B256::from_str(
                    "0xc008e9f9bb92057dd0035496fbf4fb54f66b4b18b370928e46d6603933054d5a",
                )?,
            },
            block_hash: B256::from_str(
                "0x88fadbb673928c61b9ede3694ae0589ac77ae38ec90a24a6e12e83f42f18c7e8",
            )?,
            block_number: 0xa74fde,
            log_index: 0x653b,
            transaction_index: 0x1f,
        });

        let serialized = serde_json::to_string(&log).unwrap();
        let deserialized: BlockLog = serde_json::from_str(&serialized).unwrap();

        assert_eq!(log, deserialized);

        Ok(())
    }

    #[test]
    fn test_block_log_partial_serde() -> anyhow::Result<()> {
        let log = BlockLog::Partial(ReceiptLog {
            inner: ExecutionLog::new_unchecked(
                Address::from_str("0000000000000000000000000000000000000011").unwrap(),
                vec![
                    B256::from_str(
                        "000000000000000000000000000000000000000000000000000000000000dead",
                    )?,
                    B256::from_str(
                        "000000000000000000000000000000000000000000000000000000000000beef",
                    )?,
                ],
                Bytes::from(hex::decode("0100ff")?),
            ),
            transaction_hash: B256::from_str(
                "0xc008e9f9bb92057dd0035496fbf4fb54f66b4b18b370928e46d6603933054d5a",
            )?,
        });

        let serialized = serde_json::to_string(&log).unwrap();
        let deserialized: BlockLog = serde_json::from_str(&serialized).unwrap();

        assert_eq!(log, deserialized);

        Ok(())
    }
}
