//! Common types and functions for integration tests

use edr_block_api::{GenesisBlockFactory as _, GenesisBlockOptions};
use edr_block_header::BlockConfig;
use edr_chain_l1::{
    chains::{l1_chain_config, L1_MAINNET_CHAIN_ID},
    L1ChainSpec,
};
use edr_chain_spec_provider::ProviderChainSpec;
use edr_primitives::B256;
use edr_provider::spec::LocalBlockchainForChainSpec;
use edr_state_api::StateDiff;

pub fn create_dummy_local_blockchain() -> LocalBlockchainForChainSpec<L1ChainSpec> {
    const DEFAULT_GAS_LIMIT: u64 = 0xffffffffffffff;
    const DEFAULT_INITIAL_BASE_FEE: u128 = 1000000000;

    let chain_config =
        l1_chain_config(L1_MAINNET_CHAIN_ID).expect("Chain config must exist for L1 mainnet");

    let block_config = BlockConfig {
        base_fee_params: chain_config.base_fee_params.clone(),
        hardfork: edr_chain_l1::Hardfork::default(),
        min_ethash_difficulty: L1ChainSpec::MIN_ETHASH_DIFFICULTY,
        scheduled_blob_params: chain_config.bpo_hardfork_schedule.clone(),
    };

    let genesis_diff = StateDiff::default();
    let genesis_block = L1ChainSpec::genesis_block(
        genesis_diff.clone(),
        block_config.clone(),
        GenesisBlockOptions {
            gas_limit: Some(DEFAULT_GAS_LIMIT),
            mix_hash: Some(B256::ZERO),
            base_fee: Some(DEFAULT_INITIAL_BASE_FEE),
            ..GenesisBlockOptions::default()
        },
    )
    .expect("Failed to create genesis block");

    LocalBlockchainForChainSpec::<L1ChainSpec>::new(
        genesis_block,
        genesis_diff,
        L1_MAINNET_CHAIN_ID,
        block_config,
    )
    .expect("Should construct without issues")
}
