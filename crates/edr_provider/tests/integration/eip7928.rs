#![cfg(feature = "test-utils")]

//! EIP-7928: Block-Level Access Lists.
//! see <https://eips.ethereum.org/EIPS/eip-7928>
//
//! From Amsterdam onward, the block header carries a `blockAccessListHash`
//! field. On earlier hardforks the field is omitted from the RPC response.

use std::sync::Arc;

use edr_chain_l1::{rpc::block::L1RpcBlock, L1ChainSpec};
use edr_primitives::{address, Address, B256, KECCAK_RLP_EMPTY_ARRAY, U256};
use edr_provider::{
    test_utils::{create_test_config, get_latest_block, mine_block, transfer_value},
    time::CurrentTime,
    NoopLogger, Provider,
};
use edr_solidity::contract_decoder::ContractDecoder;
use parking_lot::RwLock;
use tokio::runtime;

const BLOCK_ACCESS_LIST_HASH_JSON_KEY: &str = "blockAccessListHash";

const SENDER: Address = address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
const RECIPIENT: Address = address!("0x70997970C51812dc3A010C7d01b50e0d17dc79C8");

fn new_provider(hardfork: edr_chain_l1::Hardfork) -> anyhow::Result<Provider<L1ChainSpec>> {
    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});

    let mut config = create_test_config();
    config.hardfork = hardfork;

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::new(RwLock::<ContractDecoder>::default()),
        CurrentTime,
    )?;

    Ok(provider)
}

#[tokio::test(flavor = "multi_thread")]
async fn block_header_includes_block_access_list_hash_on_amsterdam() -> anyhow::Result<()> {
    let provider = new_provider(edr_chain_l1::Hardfork::AMSTERDAM)?;

    mine_block(&provider);
    let block_json = get_latest_block(&provider);

    assert!(
        block_json.get(BLOCK_ACCESS_LIST_HASH_JSON_KEY).is_some(),
        "Amsterdam block header should include {BLOCK_ACCESS_LIST_HASH_JSON_KEY}, block: {block_json}"
    );

    Ok(())
}

// A block that modified state must carry a non-empty block access list hash.
#[tokio::test(flavor = "multi_thread")]
async fn block_access_list_hash_is_non_empty_for_block_with_transactions() -> anyhow::Result<()> {
    let provider = new_provider(edr_chain_l1::Hardfork::AMSTERDAM)?;

    transfer_value(&provider, SENDER, RECIPIENT, U256::from(1000));
    let block_json = get_latest_block(&provider);

    let block: L1RpcBlock<B256> = serde_json::from_value(block_json)?;
    let block_access_list_hash = block
        .block_access_list_hash
        .expect("Amsterdam block should include a block access list hash");

    assert_ne!(
        block_access_list_hash, KECCAK_RLP_EMPTY_ARRAY,
        "a block that modified state should not carry the empty-list block access list hash"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn block_header_omits_block_access_list_hash_before_amsterdam() -> anyhow::Result<()> {
    let provider = new_provider(edr_chain_l1::Hardfork::OSAKA)?;

    mine_block(&provider);
    let block_json = get_latest_block(&provider);

    assert!(
        block_json.get(BLOCK_ACCESS_LIST_HASH_JSON_KEY).is_none(),
        "pre-Amsterdam block header should not include {BLOCK_ACCESS_LIST_HASH_JSON_KEY}. block: {block_json}"
    );

    let block: L1RpcBlock<B256> = serde_json::from_value(block_json)?;
    assert_eq!(
        block.block_access_list_hash, None,
        "pre-Amsterdam block_access_list_hash should be absent"
    );

    Ok(())
}

#[cfg(feature = "test-remote")]
mod fork {
    use edr_primitives::HashMap;
    use edr_provider::{
        config::ForkConfig,
        test_utils::{create_test_config_with, MinimalProviderConfig},
    };
    use edr_test_utils::env::json_rpc_url_provider;

    use super::*;

    /// Mines a state-modifying block and returns its block access list hash.
    fn mine_and_get_bal_hash(provider: &Provider<L1ChainSpec>) -> B256 {
        transfer_value(provider, SENDER, RECIPIENT, U256::from(1000));
        let block: L1RpcBlock<B256> =
            serde_json::from_value(get_latest_block(provider)).expect("block should deserialize");
        block
            .block_access_list_hash
            .expect("Amsterdam block should include a block access list hash")
    }

    // The simulated block access list hash must be unique per block, including in
    // forked mode.
    #[tokio::test(flavor = "multi_thread")]
    async fn block_access_list_hash_differs_across_forked_amsterdam_blocks() -> anyhow::Result<()> {
        let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
        let subscriber = Box::new(|_event| {});

        let mut config =
            create_test_config_with(MinimalProviderConfig::fork_with_accounts(ForkConfig {
                block_number: Some(20_384_300),
                cache_dir: edr_defaults::CACHE_DIR.into(),
                chain_overrides: HashMap::default(),
                http_headers: None,
                url: json_rpc_url_provider::ethereum_mainnet(),
            }));
        config.hardfork = edr_chain_l1::Hardfork::AMSTERDAM;

        let provider = Provider::new(
            runtime::Handle::current(),
            logger,
            subscriber,
            config,
            Arc::new(RwLock::<ContractDecoder>::default()),
            CurrentTime,
        )?;

        let first_block = mine_and_get_bal_hash(&provider);
        let second_block = mine_and_get_bal_hash(&provider);

        assert_ne!(first_block, KECCAK_RLP_EMPTY_ARRAY);
        assert_ne!(second_block, KECCAK_RLP_EMPTY_ARRAY);
        assert_ne!(
            first_block, second_block,
            "forked blocks that modify state should have distinct block access list hashes"
        );

        Ok(())
    }
}
