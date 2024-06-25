use std::ops::Deref;

use alloy_rlp::BufMut;

use super::{Receipt, TransactionReceipt};
use crate::{log::FilterLog, B256};

/// Type for a receipt that's included in a block.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct BlockReceipt<ExecutionReceiptT: Receipt<FilterLog>> {
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub inner: TransactionReceipt<ExecutionReceiptT, FilterLog>,
    /// Hash of the block that this is part of
    pub block_hash: B256,
    /// Number of the block that this is part of
    #[cfg_attr(feature = "serde", serde(with = "crate::serde::u64"))]
    pub block_number: u64,
}

impl<ExecutionReceiptT: Receipt<FilterLog>> Deref for BlockReceipt<ExecutionReceiptT> {
    type Target = TransactionReceipt<ExecutionReceiptT, FilterLog>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<ExecutionReceiptT> alloy_rlp::Encodable for BlockReceipt<ExecutionReceiptT>
where
    ExecutionReceiptT: Receipt<FilterLog> + alloy_rlp::Encodable,
{
    fn encode(&self, out: &mut dyn BufMut) {
        self.inner.encode(out);
    }

    fn length(&self) -> usize {
        self.inner.length()
    }
}

#[cfg(all(test, feature = "serde"))]
mod test {
    use std::sync::OnceLock;

    use assert_json_diff::assert_json_eq;
    use serde_json::json;

    use super::*;
    use crate::{
        chain_spec::L1ChainSpec,
        receipt,
        result::{ExecutionResult, Output, SuccessReason},
        signature::Fakeable,
        transaction::{self, TxKind},
        AccessList, Address, Bloom, Bytes, U256,
    };

    #[test]
    fn test_block_receipt_serde() {
        let execution_result = ExecutionResult::<L1ChainSpec>::Success {
            reason: SuccessReason::Stop,
            gas_used: 100,
            gas_refunded: 0,
            logs: Vec::new(),
            output: Output::Call(Bytes::new()),
        };

        let transaction: transaction::Signed = transaction::signed::Eip1559 {
            chain_id: 1,
            nonce: 1,
            max_priority_fee_per_gas: U256::ZERO,
            max_fee_per_gas: U256::from(100u64),
            gas_limit: 100,
            kind: TxKind::Call(Address::default()),
            value: U256::ZERO,
            input: Bytes::new(),
            access_list: AccessList::default(),
            signature: Fakeable::fake(Address::default(), None),
            hash: OnceLock::new(),
            rlp_encoding: OnceLock::new(),
        }
        .into();

        let receipt = BlockReceipt {
            inner: TransactionReceipt::new(
                receipt::Execution::Eip2718(receipt::execution::Eip2718 {
                    status: true,
                    cumulative_gas_used: 100,
                    logs_bloom: Bloom::ZERO,
                    logs: vec![],
                    transaction_type: crate::transaction::Type::Eip1559,
                }),
                &transaction,
                &execution_result,
                0,
                U256::ZERO,
            ),
            block_hash: B256::default(),
            block_number: 1,
        };

        let serialized = serde_json::to_string(&receipt).unwrap();
        let deserialized = serde_json::from_str(&serialized).unwrap();

        assert_eq!(receipt, deserialized);
    }

    #[test]
    fn test_matches_hardhat_serialization() -> anyhow::Result<()> {
        // Generated with the "Hardhat Network provider eth_getTransactionReceipt should
        // return the right values for successful txs" hardhat-core test.
        let receipt_from_hardhat = json!({
          "transactionHash": "0x08d14db1a6253234f7efc94fc661f52b708882552af37ebf4f5cd904618bb208",
          "transactionIndex": "0x0",
          "blockHash": "0x404b3b3ed507ff47178e9ca9d7757165050180091e1cc17de7981871a6e5785a",
          "blockNumber": "0x2",
          "from": "0xbe862ad9abfe6f22bcb087716c7d89a26051f74c",
          "to": "0x61de9dc6f6cff1df2809480882cfd3c2364b28f7",
          "cumulativeGasUsed": "0xaf91",
          "gasUsed": "0xaf91",
          "contractAddress": null,
          "logs": [
            {
              "removed": false,
              "logIndex": "0x0",
              "transactionIndex": "0x0",
              "transactionHash": "0x08d14db1a6253234f7efc94fc661f52b708882552af37ebf4f5cd904618bb208",
              "blockHash": "0x404b3b3ed507ff47178e9ca9d7757165050180091e1cc17de7981871a6e5785a",
              "blockNumber": "0x2",
              "address": "0x61de9dc6f6cff1df2809480882cfd3c2364b28f7",
              "data": "0x000000000000000000000000000000000000000000000000000000000000000a",
              "topics": [
                "0x3359f789ea83a10b6e9605d460de1088ff290dd7b3c9a155c896d45cf495ed4d",
                "0x0000000000000000000000000000000000000000000000000000000000000000"
              ]
            }
          ],
          "logsBloom": "0x00000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000400000000000000000020000000000000000000800000002000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000000",
          "type": "0x2",
          "status": "0x1",
          "effectiveGasPrice": "0x699e6346"
        });

        let deserialized: BlockReceipt<receipt::Execution<FilterLog>> =
            serde_json::from_value(receipt_from_hardhat.clone())?;

        let serialized = serde_json::to_value(deserialized)?;
        assert_json_eq!(receipt_from_hardhat, serialized);

        Ok(())
    }
}
