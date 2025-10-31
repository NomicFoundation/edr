/// Macro to implement common blockchain tests for different blockchain
/// implementations.
#[macro_export]
macro_rules! impl_test_blockchain_tests {
    ($name:ident: $error_ty:ty => $blockchain_constructor:expr) => {
        $crate::paste::item! {
            #[tokio::test(flavor = "multi_thread")]
            #[serial]
            async fn [<test_get_last_block_from_ $name _blockchain>]() -> anyhow::Result<()> {
                let mut blockchain = $blockchain_constructor;

                let last_block_number = blockchain.last_block_number();

                let last_block: std::sync::Arc<$crate::DynSyncBlock<L1ChainSpec>> = blockchain.last_block()?;
                assert_eq!(last_block.block_header().number, last_block_number);

                let next_block = $crate::create_dummy_block(&blockchain);
                let expected: $crate::BlockAndTotalDifficultyForChainSpec<L1ChainSpec> = blockchain.insert_block(next_block, StateDiff::default())?;

                let last_block: std::sync::Arc<$crate::DynSyncBlock<L1ChainSpec>> = blockchain.last_block()?;
                assert_eq!(
                    last_block.block_hash(),
                    expected.block.block_hash()
                );

                Ok(())
            }

            #[tokio::test(flavor = "multi_thread")]
            #[serial]
            async fn [<test_block_by_hash_some_from_ $name _blockchain>]() {
                let mut blockchain = $blockchain_constructor;

                let next_block = $crate::create_dummy_block(&blockchain);
                let expected: $crate::BlockAndTotalDifficultyForChainSpec<L1ChainSpec> = blockchain
                    .insert_block(next_block, StateDiff::default())
                    .expect("Failed to insert block");

                let found_block: std::sync::Arc<$crate::DynSyncBlock<L1ChainSpec>> =
                    blockchain
                        .block_by_hash(expected.block.block_hash())
                        .unwrap()
                        .unwrap();

                assert_eq!(
                    found_block.block_hash(),
                    expected.block.block_hash()
                );
            }

            #[tokio::test(flavor = "multi_thread")]
            #[serial]
            async fn [<test_block_by_hash_none_from_ $name _blockchain>]() {
                let blockchain = $blockchain_constructor;

                let found_block: Option<std::sync::Arc<$crate::DynSyncBlock<L1ChainSpec>>> = blockchain.block_by_hash(&B256::ZERO).unwrap();
                assert!(found_block.is_none());
            }

            #[tokio::test(flavor = "multi_thread")]
            #[serial]
            async fn [<test_block_by_number_some_from_ $name _blockchain>]() {
                let mut blockchain = $blockchain_constructor;

                let next_block = $crate::create_dummy_block(&blockchain);
                let expected: $crate::BlockAndTotalDifficultyForChainSpec<L1ChainSpec> = blockchain
                    .insert_block(next_block, StateDiff::default())
                    .expect("Failed to insert block");

                let found_block: std::sync::Arc<$crate::DynSyncBlock<L1ChainSpec>> =
                    blockchain
                        .block_by_number(expected.block.block_header().number)
                        .unwrap()
                        .unwrap();

                assert_eq!(
                    found_block.block_hash(),
                    expected.block.block_hash()
            );
            }

            #[tokio::test]
            #[serial]
            async fn [<test_block_by_number_none_from_ $name _blockchain>]() {
                let blockchain = $blockchain_constructor;

                let next_block_number = blockchain.last_block_number() + 1;
                let found_block: Option<std::sync::Arc<$crate::DynSyncBlock<L1ChainSpec>>> = blockchain.block_by_number(next_block_number).unwrap();
                assert!(found_block.is_none());
            }

            #[tokio::test(flavor = "multi_thread")]
            #[serial]
            async fn [<test_insert_block_multiple_from_ $name _blockchain>]() -> anyhow::Result<()> {
                let mut blockchain = $blockchain_constructor;

                let one = $crate::create_dummy_block(&blockchain);
                let one: $crate::BlockAndTotalDifficultyForChainSpec<L1ChainSpec> = blockchain.insert_block(one, StateDiff::default())?;

                let two = $crate::create_dummy_block(&blockchain);
                let two: $crate::BlockAndTotalDifficultyForChainSpec<L1ChainSpec> = blockchain.insert_block(two, StateDiff::default())?;

                let found_block: std::sync::Arc<$crate::DynSyncBlock<L1ChainSpec>> =
                    blockchain
                        .block_by_number(one.block.block_header().number)?
                        .unwrap();

                assert_eq!(
                    found_block.block_hash(),
                    one.block.block_hash()
                );

                let found_block: std::sync::Arc<$crate::DynSyncBlock<L1ChainSpec>> =
                    blockchain
                        .block_by_number(two.block.block_header().number)?
                        .unwrap();

                assert_eq!(
                    found_block.block_hash(),
                    two.block.block_hash()
                );

                Ok(())
            }

            #[tokio::test(flavor = "multi_thread")]
            #[serial]
            async fn [<test_insert_block_invalid_block_number_from_ $name _blockchain>]() {
                let mut blockchain = $blockchain_constructor;

                let next_block_number = blockchain.last_block_number() + 1;
                let invalid_block_number = next_block_number + 1;

                let invalid_block =
                    $crate::create_dummy_block_with_number(&blockchain, invalid_block_number);
                let error = InsertBlock::<$crate::DynSyncBlock<L1ChainSpec>, _, _>::insert_block(
                    &mut blockchain,
                    invalid_block,
                    StateDiff::default()
                )
                    .expect_err("Should fail to insert block");

                if let $error_ty::InvalidNextBlock(BlockValidityError::InvalidBlockNumber { actual, expected }) = error {
                    assert_eq!(actual, invalid_block_number);
                    assert_eq!(expected, next_block_number);
                } else {
                    panic!("Unexpected error: {error:?}");
                }
            }

            #[tokio::test(flavor = "multi_thread")]
            #[serial]
            async fn [<test_insert_block_invalid_parent_hash_from_ $name _blockchain>]() {
                let mut blockchain = $blockchain_constructor;

                const INVALID_BLOCK_HASH: B256 = B256::ZERO;
                let next_block_number = blockchain.last_block_number() + 1;

                let one = $crate::create_dummy_block_with_hash(
                    &blockchain,
                    next_block_number,
                    INVALID_BLOCK_HASH,
                );
                let error = InsertBlock::<$crate::DynSyncBlock<L1ChainSpec>, _, _>::insert_block(
                    &mut blockchain,
                    one,
                    StateDiff::default()
                )
                    .expect_err("Should fail to insert block");

                if let $error_ty::InvalidNextBlock(BlockValidityError::InvalidParentHash { actual, expected }) = error {
                    let last_block: std::sync::Arc<$crate::DynSyncBlock<L1ChainSpec>> = blockchain.last_block().unwrap();

                    assert_eq!(actual, INVALID_BLOCK_HASH);
                    assert_eq!(expected, *last_block.block_hash());
                } else {
                    panic!("Unexpected error: {error:?}");
                }
            }

            #[tokio::test(flavor = "multi_thread")]
            #[serial]
            async fn [<test_logs_local_for_ $name _blockchain>]() -> anyhow::Result<()> {
                fn assert_eq_logs(actual: &[FilterLog], expected: &[ExecutionLog]) {
                    assert_eq!(expected.len(), actual.len());

                    for (log, filter_log) in expected.iter().zip(actual.iter()) {
                        assert_eq!(log.address, filter_log.address);
                        assert_eq!(log.topics(), filter_log.topics());
                        assert_eq!(log.data, filter_log.data);
                    }
                }

                let mut blockchain = $blockchain_constructor;

                let last_block_number = blockchain.last_block_number();

                let $crate::DummyBlockAndTransaction {
                    block: one,
                    transaction_receipt,
                    ..
                } = $crate::insert_dummy_block_with_transaction(&mut blockchain)?;

                let filtered_logs = blockchain.logs(
                    one.block_header().number,
                    one.block_header().number,
                    &HashSet::default(),
                    &[],
                )?;

                assert_eq_logs(&filtered_logs, transaction_receipt.transaction_logs());

                let logs = transaction_receipt.transaction_logs().iter();
                let $crate::DummyBlockAndTransaction {
                    block: two,
                    transaction_receipt,
                    ..
                } = $crate::insert_dummy_block_with_transaction(&mut blockchain)?;

                let logs: Vec<ExecutionLog> = logs
                    .chain(transaction_receipt.transaction_logs().iter())
                    .cloned()
                    .collect();

                let filtered_logs = blockchain.logs(
                    one.block_header().number,
                    two.block_header().number,
                    &HashSet::default(),
                    &[],
                )?;

                assert_eq_logs(&filtered_logs, &logs);

                // Removed blocks should not have logs
                blockchain.revert_to_block(last_block_number)?;

                let filtered_logs = blockchain.logs(
                    one.block_header().number,
                    two.block_header().number,
                    &HashSet::default(),
                    &[],
                )?;

                assert!(filtered_logs.is_empty());

                Ok(())
            }

            #[tokio::test(flavor = "multi_thread")]
            #[serial]
            async fn [<test_revert_to_block_local_for_ $name _blockchain>]() -> anyhow::Result<()> {
                let mut blockchain = $blockchain_constructor;

                let last_block: std::sync::Arc<$crate::DynSyncBlock<L1ChainSpec>> = blockchain.last_block()?;

                let one = $crate::create_dummy_block(&blockchain);
                let one: $crate::BlockAndTotalDifficultyForChainSpec<L1ChainSpec> = blockchain.insert_block(one, StateDiff::default())?;

                let two = $crate::create_dummy_block(&blockchain);
                let two: $crate::BlockAndTotalDifficultyForChainSpec<L1ChainSpec> = blockchain.insert_block(two, StateDiff::default())?;

                blockchain.revert_to_block(last_block.block_header().number)?;

                // Last block still exists
                let reverted_block: std::sync::Arc<$crate::DynSyncBlock<L1ChainSpec>> = blockchain.last_block()?;
                assert_eq!(
                    reverted_block.block_hash(),
                    last_block.block_hash()
                );
                assert_eq!(last_block.block_header().number, blockchain.last_block_number());

                let found_block: std::sync::Arc<$crate::DynSyncBlock<L1ChainSpec>> = blockchain
                    .block_by_hash(last_block.block_hash())?
                    .unwrap();

                assert_eq!(
                    found_block.block_hash(),
                    last_block.block_hash()
                );

                // Blocks 1 and 2 are gone
                let block_one: Option<std::sync::Arc<$crate::DynSyncBlock<L1ChainSpec>>> = blockchain
                    .block_by_number(one.block.block_header().number)?;

                assert!(block_one.is_none());

                let block_two: Option<std::sync::Arc<$crate::DynSyncBlock<L1ChainSpec>>> = blockchain
                    .block_by_number(two.block.block_header().number)?;

                assert!(block_two.is_none());

                let block_one: Option<std::sync::Arc<$crate::DynSyncBlock<L1ChainSpec>>> = blockchain
                    .block_by_hash(one.block.block_hash())?;

                assert!(block_one.is_none());

                let block_two: Option<std::sync::Arc<$crate::DynSyncBlock<L1ChainSpec>>> = blockchain
                    .block_by_hash(two.block.block_hash())?;

                assert!(block_two.is_none());

                // Can insert a new block after reverting
                let new = $crate::create_dummy_block(&blockchain);
                let new: $crate::BlockAndTotalDifficultyForChainSpec<L1ChainSpec> = blockchain.insert_block(new.clone(), StateDiff::default())?;

                let last_block: std::sync::Arc<$crate::DynSyncBlock<L1ChainSpec>> = blockchain.last_block()?;
                assert_eq!(
                    last_block.block_hash(),
                    new.block.block_hash()
                );

                Ok(())
            }

            #[tokio::test]
            #[serial]
            async fn [<test_revert_to_block_invalid_number_for_ $name _blockchain>]() {
                let mut blockchain = $blockchain_constructor;

                let next_block_number = blockchain.last_block_number() + 1;
                let error = blockchain
                    .revert_to_block(next_block_number)
                    .expect_err("Should fail to insert block");

                assert!(matches!(error, $error_ty::UnknownBlockNumber));
            }

            #[tokio::test(flavor = "multi_thread")]
            #[serial]
            async fn [<test_block_total_difficulty_by_hash_for_ $name _blockchain>]() {
                let mut blockchain = $blockchain_constructor;

                let last_block: std::sync::Arc<$crate::DynSyncBlock<L1ChainSpec>> = blockchain.last_block().unwrap();
                let last_block_header = last_block.block_header();

                let one = $crate::create_dummy_block_with_difficulty(
                    &blockchain,
                    last_block_header.number + 1,
                    1000,
                );
                let one: $crate::BlockAndTotalDifficultyForChainSpec<L1ChainSpec> = blockchain.insert_block(one, StateDiff::default()).unwrap();

                let two = $crate::create_dummy_block_with_difficulty(
                    &blockchain,
                    last_block_header.number + 2,
                    2000,
                );
                let two: $crate::BlockAndTotalDifficultyForChainSpec<L1ChainSpec> = blockchain.insert_block(two, StateDiff::default()).unwrap();

                let last_block_difficulty = blockchain
                    .total_difficulty_by_hash(last_block.block_hash())
                    .unwrap()
                    .expect("total difficulty must exist");

                assert_eq!(
                    blockchain
                        .total_difficulty_by_hash(one.block.block_hash())
                        .unwrap(),
                    Some(last_block_difficulty + one.block.block_header().difficulty)
                );

                assert_eq!(
                    blockchain
                        .total_difficulty_by_hash(two.block.block_hash())
                        .unwrap(),
                    Some(
                        last_block_difficulty
                            + one.block.block_header().difficulty
                            + two.block.block_header().difficulty
                    )
                );

                blockchain
                    .revert_to_block(one.block.block_header().number)
                    .unwrap();

                // Block 1 has a total difficulty
                assert_eq!(
                    blockchain
                        .total_difficulty_by_hash(one.block.block_hash())
                        .unwrap(),
                    Some(last_block_difficulty + one.block.block_header().difficulty)
                );

                // Block 2 no longer stores a total difficulty
                assert!(blockchain
                    .total_difficulty_by_hash(two.block.block_hash())
                    .unwrap()
                    .is_none());
            }

            #[tokio::test(flavor = "multi_thread")]
            #[serial]
            async fn [<test_block_total_difficulty_by_hash_invalid_hash_for_ $name _blockchain>]() {
                let blockchain = $blockchain_constructor;

                let difficulty = blockchain.total_difficulty_by_hash(&B256::ZERO).unwrap();

                assert!(difficulty.is_none());
            }

            #[tokio::test(flavor = "multi_thread")]
            #[serial]
            async fn [<test_block_by_transaction_hash_local_from_ $name _blockchain>]() -> anyhow::Result<()> {
                let mut blockchain = $blockchain_constructor;

                let previous_block_number = blockchain.last_block_number();

                let $crate::DummyBlockAndTransaction {
                    block: mined_block,
                    transaction_hash,
                    ..
                } = $crate::insert_dummy_block_with_transaction(&mut blockchain)?;
                let block: Option<std::sync::Arc<$crate::DynSyncBlock<L1ChainSpec>>> = blockchain.block_by_transaction_hash(&transaction_hash)?;

                assert!(block.is_some());

                let block = block.unwrap();
                assert!(std::sync::Arc::ptr_eq(&block, &mined_block));

                let transactions = block.transactions();
                assert_eq!(transactions.len(), 1);
                assert_eq!(*transactions[0].transaction_hash(), transaction_hash);

                blockchain.revert_to_block(previous_block_number)?;

                // Once reverted, the block is no longer available
                let block: Option<std::sync::Arc<$crate::DynSyncBlock<L1ChainSpec>>> = blockchain.block_by_transaction_hash(&transaction_hash)?;
                assert!(block.is_none());

                Ok(())
            }

            #[tokio::test(flavor = "multi_thread")]
            #[serial]
            async fn [<test_block_by_transaction_hash_unknown_from_ $name _blockchain>]() -> anyhow::Result<()> {
                let blockchain = $blockchain_constructor;

                let transaction = dummy_eip155_transaction(Address::random(), 0)?;

                let block: Option<std::sync::Arc<$crate::DynSyncBlock<L1ChainSpec>>> = blockchain.block_by_transaction_hash(transaction.transaction_hash())?;
                assert!(block.is_none());

                Ok(())
            }

            #[tokio::test(flavor = "multi_thread")]
            #[serial]
            async fn [<test_receipt_by_transaction_hash_local_from_ $name _blockchain>]() -> anyhow::Result<()> {
                let mut blockchain = $blockchain_constructor;

                let previous_block_number = blockchain.last_block_number();

                let $crate::DummyBlockAndTransaction {
                    transaction_hash,
                    transaction_receipt,
                    ..
                } = $crate::insert_dummy_block_with_transaction(&mut blockchain)?;
                let receipt = blockchain.receipt_by_transaction_hash(&transaction_hash)?;

                assert!(receipt.is_some());

                let receipt = receipt.unwrap();
                assert_eq!(
                    receipt.transaction_hash,
                    transaction_receipt.transaction_hash
                );
                assert_eq!(
                    receipt.transaction_index,
                    transaction_receipt.transaction_index
                );
                assert_eq!(receipt.from, transaction_receipt.from);
                assert_eq!(receipt.to, transaction_receipt.to);
                assert_eq!(
                    receipt.contract_address,
                    transaction_receipt.contract_address
                );
                assert_eq!(receipt.gas_used, transaction_receipt.gas_used);
                assert_eq!(
                    receipt.effective_gas_price,
                    transaction_receipt.effective_gas_price
                );

                blockchain.revert_to_block(previous_block_number)?;

                // Once reverted, the receipt is no longer available
                let receipt = blockchain.receipt_by_transaction_hash(&transaction_hash)?;
                assert!(receipt.is_none());

                Ok(())
            }

            #[tokio::test(flavor = "multi_thread")]
            #[serial]
            async fn [<test_receipt_by_transaction_hash_unknown_from_ $name _blockchain>]() -> anyhow::Result<()> {
                let blockchain = $blockchain_constructor;

                let transaction = dummy_eip155_transaction(Address::random(), 0)?;

                let receipt =
                    blockchain.receipt_by_transaction_hash(transaction.transaction_hash())?;
                assert!(receipt.is_none());

                Ok(())
            }
        }
    };
}
