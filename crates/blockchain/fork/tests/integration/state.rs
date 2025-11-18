#![cfg(feature = "test-remote")]

use edr_blockchain_api::{GetBlockchainBlock as _, StateAtBlock as _};
use edr_state_api::irregular::IrregularState;
use serial_test::serial;

use crate::common::create_dummy_forked_blockchain;

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn state_at_block_number_historic() {
    let blockchain = create_dummy_forked_blockchain(None).await;
    let irregular_state = IrregularState::default();

    let genesis_block = blockchain
        .block_by_number(0)
        .expect("Failed to retrieve block")
        .expect("Block should exist");

    let state = blockchain
        .state_at_block_number(0, irregular_state.state_overrides())
        .unwrap();
    assert_eq!(
        state.state_root().expect("State root should be returned"),
        genesis_block.block_header().state_root
    );
}
