#[macro_export]
macro_rules! impl_execution_receipt_tests {
    ($(
        $name:ident => $receipt:expr,
    )+) => {
        $(
            paste::item! {
                #[test]
                fn [<typed_receipt_rpc_receipt_roundtrip_ $name>]() -> anyhow::Result<()> {
                    use std::marker::PhantomData;

                    use edr_eth::{
                        log::{FilterLog, FullBlockLog, ReceiptLog},
                        receipt::{BlockReceipt, MapReceiptLogs as _, TransactionReceipt},
                        Address, B256, U256,
                    };

                    use $crate::receipt::ToRpcReceipt as _;

                    let block_hash = B256::random();
                    let block_number = 10u64;
                    let transaction_hash = B256::random();
                    let transaction_index = 5u64;

                    let execution_receipt = $receipt;

                    let mut log_index = 0;
                    let execution_receipt = execution_receipt.map_logs(|log| FilterLog {
                        inner: FullBlockLog {
                            inner: ReceiptLog {
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


                    let transaction_receipt = TransactionReceipt {
                        inner: execution_receipt,
                        transaction_hash,
                        transaction_index,
                        from: Address::random(),
                        to: Some(Address::random()),
                        contract_address: Some(Address::random()),
                        gas_used: 100,
                        effective_gas_price: Some(U256::from(100u64)),
                        phantom: PhantomData,
                    };
                    let block_receipt = BlockReceipt {
                        inner: transaction_receipt,
                        block_hash,
                        block_number,
                    };

                    let rpc_receipt = block_receipt.to_rpc_receipt(Default::default());

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
