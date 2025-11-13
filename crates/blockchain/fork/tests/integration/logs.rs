#![cfg(feature = "test-remote")]

use std::str::FromStr as _;

use edr_blockchain_api::GetBlockchainLogs as _;
use edr_primitives::{Address, HashSet};
use edr_test_blockchain::insert_dummy_block_with_transaction;
use serial_test::serial;

use crate::common::{create_dummy_forked_blockchain, REMOTE_BLOCK_NUMBER};

/// See results at <https://api.etherscan.io/api?module=logs&action=getLogs&fromBlock=10496585&toBlock=10496585&address=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2>
#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn logs_remote() -> anyhow::Result<()> {
    let blockchain = create_dummy_forked_blockchain(None).await;

    let address = Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")?;
    let addresses = [address].into_iter().collect();

    let logs = blockchain.logs(REMOTE_BLOCK_NUMBER, REMOTE_BLOCK_NUMBER, &addresses, &[])?;

    assert_eq!(logs.len(), 12);

    let expected = [1, 4, 13, 14, 17, 20, 27, 30, 41, 42, 139, 140];
    logs.iter().zip(expected).for_each(|(log, expected_index)| {
        assert_eq!(log.log_index, expected_index);
    });

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn logs_remote_and_local() -> anyhow::Result<()> {
    let mut blockchain = create_dummy_forked_blockchain(Some(REMOTE_BLOCK_NUMBER)).await;

    insert_dummy_block_with_transaction(&mut blockchain)?;
    insert_dummy_block_with_transaction(&mut blockchain)?;

    let logs = blockchain.logs(
        REMOTE_BLOCK_NUMBER,
        REMOTE_BLOCK_NUMBER + 1,
        &HashSet::default(),
        &[],
    )?;

    assert_eq!(logs.len(), 207);

    Ok(())
}
