#![cfg(feature = "test-utils")]

//! EIP-7928: Block-Level Access Lists.
//! see <https://eips.ethereum.org/EIPS/eip-7928>
//
//! From Amsterdam onward, the block header carries a `blockAccessListHash`
//! field. On earlier hardforks the field is omitted from the RPC response.

use std::sync::Arc;

use edr_chain_l1::{rpc::block::L1RpcBlock, L1ChainSpec};
use edr_eth::PreEip1898BlockSpec;
use edr_primitives::B256;
use edr_provider::{
    test_utils::create_test_config, time::CurrentTime, MethodInvocation, NoopLogger, Provider,
    ProviderRequest,
};
use edr_solidity::contract_decoder::ContractDecoder;
use parking_lot::RwLock;
use tokio::runtime;

const BLOCK_ACCESS_LIST_HASH_JSON_KEY: &str = "blockAccessListHash";

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

/// Mines an empty block and returns the raw JSON of the resulting latest block.
fn mine_and_get_latest_block(provider: &Provider<L1ChainSpec>) -> serde_json::Value {
    provider
        .handle_request(ProviderRequest::with_single(MethodInvocation::EvmMine(
            None,
        )))
        .expect("evm_mine should succeed");

    provider
        .handle_request(ProviderRequest::with_single(
            MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::latest(), false),
        ))
        .expect("eth_getBlockByNumber should succeed")
        .result
}

#[tokio::test(flavor = "multi_thread")]
async fn block_header_includes_block_access_list_hash_on_amsterdam() -> anyhow::Result<()> {
    let provider = new_provider(edr_chain_l1::Hardfork::AMSTERDAM)?;

    let block_json = mine_and_get_latest_block(&provider);

    assert!(
        block_json.get(BLOCK_ACCESS_LIST_HASH_JSON_KEY).is_some(),
        "Amsterdam block header should include {BLOCK_ACCESS_LIST_HASH_JSON_KEY}, block: {block_json}"
    );

    let block: L1RpcBlock<B256> = serde_json::from_value(block_json)?;
    assert_eq!(
        block.block_access_list_hash,
        Some(B256::ZERO),
        "Amsterdam block_access_list_hash should be present"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn block_header_omits_block_access_list_hash_before_amsterdam() -> anyhow::Result<()> {
    let provider = new_provider(edr_chain_l1::Hardfork::OSAKA)?;

    let block_json = mine_and_get_latest_block(&provider);

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
