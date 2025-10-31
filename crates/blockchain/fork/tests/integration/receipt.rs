use std::str::FromStr as _;

use edr_blockchain_api::ReceiptByTransactionHash as _;
use edr_primitives::B256;
use serial_test::serial;

use crate::common::{
    create_dummy_forked_blockchain, REMOTE_BLOCK_FIRST_TRANSACTION_HASH, REMOTE_BLOCK_HASH,
};

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn receipt_by_transaction_hash_remote() -> anyhow::Result<()> {
    let blockchain = create_dummy_forked_blockchain(None).await;

    let transaction_hash = B256::from_str(REMOTE_BLOCK_FIRST_TRANSACTION_HASH)?;
    let receipt = blockchain.receipt_by_transaction_hash(&transaction_hash)?;

    assert!(receipt.is_some());

    let receipt = receipt.unwrap();
    assert_eq!(receipt.transaction_hash, transaction_hash);
    assert_eq!(receipt.block_hash, B256::from_str(REMOTE_BLOCK_HASH)?);

    Ok(())
}
