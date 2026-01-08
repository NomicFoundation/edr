//! Common types and functions for integration tests

use std::sync::Arc;

use edr_chain_l1::L1ChainSpec;
use edr_chain_spec_provider::ProviderChainSpec as _;
use edr_provider::spec::ForkedBlockchainForChainSpec;
use edr_rpc_eth::client::EthRpcClientForChainSpec;
use edr_state_api::irregular::IrregularState;
use edr_test_utils::env::json_rpc_url_provider;
use edr_utils::random::RandomHashGenerator;
use parking_lot::Mutex;

pub const REMOTE_BLOCK_NUMBER: u64 = 10_496_585;

pub const REMOTE_BLOCK_HASH: &str =
    "0x71d5e7c8ff9ea737034c16e333a75575a4a94d29482e0c2b88f0a6a8369c1812";

pub const REMOTE_BLOCK_FIRST_TRANSACTION_HASH: &str =
    "0xed0b0b132bd693ef34a72084f090df07c5c3a2ec019d76316da040d4222cdfb8";

pub const REMOTE_BLOCK_LAST_TRANSACTION_HASH: &str =
    "0xd809fb6f7060abc8de068c7a38e9b2b04530baf0cc4ce9a2420d59388be10ee7";

pub async fn create_dummy_forked_blockchain(
    fork_block_number: Option<u64>,
) -> ForkedBlockchainForChainSpec<L1ChainSpec> {
    let rpc_client = EthRpcClientForChainSpec::<L1ChainSpec>::new(
        &json_rpc_url_provider::ethereum_mainnet(),
        edr_defaults::CACHE_DIR.into(),
        None,
    )
    .expect("url ok");

    let mut irregular_state = IrregularState::default();
    ForkedBlockchainForChainSpec::<L1ChainSpec>::new(
        edr_chain_l1::Hardfork::default(),
        tokio::runtime::Handle::current(),
        Arc::new(rpc_client),
        &mut irregular_state,
        Arc::new(Mutex::new(RandomHashGenerator::with_seed(
            edr_defaults::STATE_ROOT_HASH_SEED,
        ))),
        L1ChainSpec::chain_configs(),
        fork_block_number,
        None,
    )
    .await
    .expect("Failed to construct forked blockchain")
}
