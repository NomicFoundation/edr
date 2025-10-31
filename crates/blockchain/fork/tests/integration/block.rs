use std::{str::FromStr as _, sync::Arc};

use edr_block_api::BlockValidityError;
use edr_blockchain_api::{
    BlockchainMetadata as _, GetBlockchainBlock as _, GetBlockchainLogs as _, InsertBlock,
    ReceiptByTransactionHash as _, RevertToBlock as _, TotalDifficultyByBlockHash as _,
};
use edr_blockchain_fork::ForkedBlockchainError;
use edr_chain_l1::L1ChainSpec;
use edr_chain_spec::ExecutableTransaction as _;
use edr_primitives::{Address, HashSet, B256};
use edr_receipt::{
    log::{ExecutionLog, FilterLog},
    ExecutionReceipt as _,
};
use edr_state_api::StateDiff;
use edr_test_blockchain::impl_test_blockchain_tests;
use edr_test_transaction::dummy_eip155_transaction;
use edr_transaction::TxKind;
use serial_test::serial;

use crate::common::{
    create_dummy_forked_blockchain, REMOTE_BLOCK_FIRST_TRANSACTION_HASH, REMOTE_BLOCK_HASH,
    REMOTE_BLOCK_LAST_TRANSACTION_HASH, REMOTE_BLOCK_NUMBER,
};

impl_test_blockchain_tests! {
    fork: ForkedBlockchainError => {
        create_dummy_forked_blockchain(None).await
    }
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn block_by_number_with_create() -> anyhow::Result<()> {
    const DAI_CREATION_BLOCK_NUMBER: u64 = 4_719_568;
    const DAI_CREATION_TRANSACTION_INDEX: usize = 85;
    const DAI_CREATION_TRANSACTION_HASH: &str =
        "0xb95343413e459a0f97461812111254163ae53467855c0d73e0f1e7c5b8442fa3";

    let blockchain = create_dummy_forked_blockchain(None).await;

    let block = blockchain
        .block_by_number(DAI_CREATION_BLOCK_NUMBER)?
        .unwrap();
    let transactions = block.transactions();

    assert_eq!(
        *transactions[DAI_CREATION_TRANSACTION_INDEX].transaction_hash(),
        B256::from_str(DAI_CREATION_TRANSACTION_HASH)?
    );
    assert_eq!(
        transactions[DAI_CREATION_TRANSACTION_INDEX].kind(),
        TxKind::Create
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn block_by_number_remote() -> anyhow::Result<()> {
    let blockchain = create_dummy_forked_blockchain(None).await;

    let block = blockchain.block_by_number(REMOTE_BLOCK_NUMBER)?.unwrap();

    let expected_hash = B256::from_str(REMOTE_BLOCK_HASH)?;
    assert_eq!(*block.block_hash(), expected_hash);

    let transactions = block.transactions();
    assert_eq!(transactions.len(), 192);
    assert_eq!(
        *transactions[0].transaction_hash(),
        B256::from_str(REMOTE_BLOCK_FIRST_TRANSACTION_HASH)?
    );
    assert_eq!(
        *transactions[transactions.len() - 1].transaction_hash(),
        B256::from_str(REMOTE_BLOCK_LAST_TRANSACTION_HASH)?
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn block_by_hash_remote() -> anyhow::Result<()> {
    let blockchain = create_dummy_forked_blockchain(None).await;

    let block = blockchain
        .block_by_hash(&B256::from_str(REMOTE_BLOCK_HASH)?)?
        .unwrap();

    assert_eq!(block.block_header().number, REMOTE_BLOCK_NUMBER);

    let transactions = block.transactions();
    assert_eq!(transactions.len(), 192);
    assert_eq!(
        *transactions[0].transaction_hash(),
        B256::from_str(REMOTE_BLOCK_FIRST_TRANSACTION_HASH)?
    );
    assert_eq!(
        *transactions[transactions.len() - 1].transaction_hash(),
        B256::from_str(REMOTE_BLOCK_LAST_TRANSACTION_HASH)?
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn block_by_caches_remote() -> anyhow::Result<()> {
    let blockchain = create_dummy_forked_blockchain(None).await;

    let block1 = blockchain.block_by_number(REMOTE_BLOCK_NUMBER)?.unwrap();
    let block2 = blockchain
        .block_by_hash(&B256::from_str(REMOTE_BLOCK_HASH)?)?
        .unwrap();
    let block3 = blockchain.block_by_number(REMOTE_BLOCK_NUMBER)?.unwrap();
    let block4 = blockchain
        .block_by_hash(&B256::from_str(REMOTE_BLOCK_HASH)?)?
        .unwrap();

    assert!(Arc::ptr_eq(&block1, &block2));
    assert!(Arc::ptr_eq(&block2, &block3));
    assert!(Arc::ptr_eq(&block3, &block4));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn block_by_transaction_hash_remote() -> anyhow::Result<()> {
    let blockchain = create_dummy_forked_blockchain(None).await;

    let block = blockchain
        .block_by_transaction_hash(&B256::from_str(REMOTE_BLOCK_FIRST_TRANSACTION_HASH)?)?;

    assert!(block.is_some());
    let block = block.unwrap();

    assert_eq!(block.block_hash(), &B256::from_str(REMOTE_BLOCK_HASH)?);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn revert_to_block_remote() -> anyhow::Result<()> {
    let mut blockchain = create_dummy_forked_blockchain(None).await;

    let last_block_number = blockchain.last_block_number();
    let error = blockchain
        .revert_to_block(last_block_number - 1)
        .unwrap_err();

    assert!(matches!(error, ForkedBlockchainError::CannotDeleteRemote));

    Ok(())
}
