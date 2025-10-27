//! Utilities for testing receipts.

// Re-export types that are used in the macros as `$crate::...`
pub use edr_chain_spec::{ChainSpec, ContextChainSpec};
pub use edr_primitives::{Address, B256};
pub use edr_receipt::{
    log::{FilterLog, FullBlockLog, ReceiptLog},
    MapReceiptLogs, TransactionReceipt,
};
pub use edr_receipt_spec::{ReceiptChainSpec, ReceiptConstructor};
pub use edr_rpc_spec::{RpcChainSpec, RpcTypeFrom};

/// Helper macro for testing serialization and deserialization roundtrips of
/// execution receipts.
#[macro_export]
macro_rules! impl_execution_receipt_serde_tests {
    ($chain_spec:ty => {
        $(
            $name:ident, $hardfork:expr => $receipt:expr,
        )+
    }) => {
        $(
            paste::item! {
                #[test]
                fn [<typed_receipt_rpc_receipt_roundtrip_ $name>]() -> anyhow::Result<()> {
                    use $crate::{MapReceiptLogs as _, RpcTypeFrom as _};

                    let block_hash = $crate::B256::random();
                    let block_number = 10u64;
                    let transaction_hash = $crate::B256::random();
                    let transaction_index = 5u64;

                    let execution_receipt = $receipt;

                    let mut log_index = 0;
                    let execution_receipt = execution_receipt.map_logs(|log| $crate::FilterLog {
                        inner: $crate::FullBlockLog {
                            inner: $crate::ReceiptLog {
                                inner: log,
                                transaction_hash,
                            },
                            block_hash,
                            block_number,
                            log_index: {
                                let index = log_index;
                                log_index += 1;
                                index
                            },
                            transaction_index,
                        },
                        removed: false,
                    });

                    let transaction_receipt = $crate::TransactionReceipt {
                        inner: execution_receipt,
                        transaction_hash,
                        transaction_index,
                        from: $crate::Address::random(),
                        to: Some($crate::Address::random()),
                        contract_address: Some($crate::Address::random()),
                        gas_used: 100,
                        effective_gas_price: Some(100),
                    };

                    // ASSUMPTION: The transaction data doesn't matter for this test, so we can use a default transaction.
                    let transaction = <$chain_spec as $crate::ChainSpec>::SignedTransaction::default();

                    let context = <$chain_spec as $crate::ContextChainSpec>::Context::default();
                    let block_receipt = <
                        <
                            $chain_spec as $crate::ReceiptChainSpec
                        >::Receipt as $crate::ReceiptConstructor<<$chain_spec as $crate::ChainSpec>::SignedTransaction>
                    >::new_receipt(&context, $hardfork, &transaction, transaction_receipt, &block_hash, block_number);

                    let rpc_receipt = <$chain_spec as $crate::RpcChainSpec>::RpcReceipt::rpc_type_from(&block_receipt, Default::default());

                    let serialized = serde_json::to_string(&rpc_receipt)?;
                    let deserialized = serde_json::from_str(&serialized)?;
                    assert_eq!(rpc_receipt, deserialized);

                    // This is necessary to ensure that the deser implementation doesn't expect a
                    // &str where a String can be passed.
                    let serialized = serde_json::to_value(&rpc_receipt)?;
                    let deserialized = serde_json::from_value(serialized)?;
                    assert_eq!(rpc_receipt, deserialized);

                    let receipt = rpc_receipt.try_into()?;
                    assert_eq!(block_receipt, receipt);

                    Ok(())
                }
            }
        )+
    };
}
