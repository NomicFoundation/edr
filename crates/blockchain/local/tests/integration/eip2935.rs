use std::collections::BTreeMap;

use edr_block_api::{GenesisBlockFactory as _, GenesisBlockOptions};
use edr_block_header::BlockConfig;
use edr_blockchain_api::StateAtBlock as _;
use edr_blockchain_fork::eips::eip2935::{
    add_history_storage_contract_to_state_diff, HISTORY_STORAGE_ADDRESS,
    HISTORY_STORAGE_UNSUPPORTED_BYTECODE,
};
use edr_blockchain_local::LocalBlockchain;
use edr_chain_l1::L1ChainSpec;
use edr_chain_spec_provider::ProviderChainSpec as _;
use edr_primitives::Bytecode;
use edr_provider::spec::LocalBlockchainForChainSpec;
use edr_state_api::StateDiff;
use edr_utils::random::RandomHashGenerator;

fn local_blockchain(
    genesis_diff: StateDiff,
) -> anyhow::Result<LocalBlockchainForChainSpec<L1ChainSpec>> {
    const CHAIN_ID: u64 = 0x7a69;
    let mut prev_randao_generator = RandomHashGenerator::with_seed(edr_defaults::MIX_HASH_SEED);

    let block_config = BlockConfig {
        base_fee_params: L1ChainSpec::default_base_fee_params(),
        hardfork: edr_chain_l1::Hardfork::PRAGUE,
        min_ethash_difficulty: L1ChainSpec::MIN_ETHASH_DIFFICULTY,
        scheduled_blob_params: None,
    };

    let genesis_block = L1ChainSpec::genesis_block(
        genesis_diff.clone(),
        block_config.clone(),
        GenesisBlockOptions {
            mix_hash: Some(prev_randao_generator.generate_next()),
            ..GenesisBlockOptions::default()
        },
    )?;

    let blockchain = LocalBlockchain::new(genesis_block, genesis_diff, CHAIN_ID, block_config)?;
    Ok(blockchain)
}

#[test]
fn test_local_blockchain_without_history() -> anyhow::Result<()> {
    let pre_prague = local_blockchain(StateDiff::default())?;

    let state = pre_prague.state_at_block_number(0, &BTreeMap::default())?;

    let history_storage_account = state.basic(HISTORY_STORAGE_ADDRESS)?;
    assert!(history_storage_account.is_none());

    Ok(())
}

#[test]
fn test_local_blockchain_with_history() -> anyhow::Result<()> {
    // Add the history storage contract to the state diff.
    let mut state_diff = StateDiff::default();
    add_history_storage_contract_to_state_diff(&mut state_diff);

    let post_prague = local_blockchain(state_diff)?;

    let state = post_prague.state_at_block_number(0, &BTreeMap::default())?;

    let history_storage_account = state
        .basic(HISTORY_STORAGE_ADDRESS)?
        .expect("Account should exist");

    let history_storage_code = history_storage_account
        .code
        .map_or_else(|| state.code_by_hash(history_storage_account.code_hash), Ok)?;

    assert_eq!(
        history_storage_code,
        Bytecode::new_raw(HISTORY_STORAGE_UNSUPPORTED_BYTECODE)
    );

    Ok(())
}
