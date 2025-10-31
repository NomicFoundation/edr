use edr_block_api::{Block as _, BlockValidityError};
use edr_blockchain_api::{
    BlockchainMetadata as _, GetBlockchainBlock as _, GetBlockchainLogs as _, InsertBlock,
    ReceiptByTransactionHash as _, RevertToBlock as _, TotalDifficultyByBlockHash as _,
};
use edr_blockchain_local::LocalBlockchainError;
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
use serial_test::serial;

impl_test_blockchain_tests! {
    local: LocalBlockchainError => {
        crate::common::create_dummy_local_blockchain()
    }
}
