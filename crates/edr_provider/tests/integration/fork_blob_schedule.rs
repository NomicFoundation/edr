#![cfg(all(feature = "test-remote", feature = "test-utils"))]

use std::sync::Arc;

use edr_chain_l1::{rpc::block::L1RpcBlock, L1ChainSpec};
use edr_eth::PreEip1898BlockSpec;
use edr_primitives::{HashMap, B256};
use edr_provider::{
    config::ForkConfig,
    test_utils::{create_test_config_with, MinimalProviderConfig},
    time::CurrentTime,
    MethodInvocation, NoopLogger, Provider, ProviderRequest,
};
use edr_solidity::contract_decoder::ContractDecoder;
use edr_test_utils::env::json_rpc_url_provider;
use parking_lot::RwLock;
use tokio::runtime;

/// Mainnet block past the BPO2 activation (timestamp 1_767_747_671).
const FORK_BLOCK_NUMBER: u64 = 25_696_896;
/// Timestamp of the forked block's on-chain child, block 25_696_897.
const CHILD_BLOCK_TIMESTAMP: u64 = 1_786_031_159;
/// `excessBlobGas` of the on-chain child, block 25_696_897. Mainnet derived it
/// from the forked block's header using the BPO2 blob params active at the
/// child's timestamp.
const CHILD_BLOCK_EXCESS_BLOB_GAS: u64 = 186_733_730;

/// Forks mainnet at `FORK_BLOCK_NUMBER`, mines a block with the same timestamp
/// as the on-chain child and returns the mined block's `excessBlobGas`.
async fn mine_child_block_excess_blob_gas(chain_id: Option<u64>) -> anyhow::Result<u64> {
    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});

    let mut config = create_test_config_with(MinimalProviderConfig::fork_empty(ForkConfig {
        block_number: Some(FORK_BLOCK_NUMBER),
        cache_dir: edr_defaults::CACHE_DIR.into(),
        chain_overrides: HashMap::default(),
        http_headers: None,
        url: json_rpc_url_provider::ethereum_mainnet(),
    }));
    config.hardfork = edr_chain_l1::Hardfork::OSAKA;
    if let Some(chain_id) = chain_id {
        config.chain_id = chain_id;
    }

    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::new(RwLock::<ContractDecoder>::default()),
        CurrentTime,
    )?;

    provider.handle_request(ProviderRequest::with_single(MethodInvocation::EvmMine(
        Some(CHILD_BLOCK_TIMESTAMP.into()),
    )))?;

    let result = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetBlockByNumber(PreEip1898BlockSpec::latest(), false),
    ))?;

    let block: L1RpcBlock<B256> = serde_json::from_value(result.result)?;
    block
        .excess_blob_gas
        .ok_or_else(|| anyhow::anyhow!("mined block should have excess blob gas"))
}

/// With the default (local) chain id, the mainnet BPO blob schedule should
/// still apply to blocks mined on top of a mainnet fork.
///
/// Currently fails: the blob schedule is looked up with the configured chain
/// id instead of the remote's, so the mined block falls back to pre-BPO Osaka
/// blob params and computes a diverging `excessBlobGas` (186_034_680).
#[tokio::test(flavor = "multi_thread")]
async fn mined_block_follows_remote_blob_schedule_with_default_chain_id() -> anyhow::Result<()> {
    let excess_blob_gas = mine_child_block_excess_blob_gas(None).await?;

    assert_eq!(excess_blob_gas, CHILD_BLOCK_EXCESS_BLOB_GAS);

    Ok(())
}

/// Control: with the remote's chain id in the config, the blob schedule lookup
/// finds mainnet's BPO params and the mined block matches the on-chain child.
#[tokio::test(flavor = "multi_thread")]
async fn mined_block_follows_remote_blob_schedule_with_remote_chain_id() -> anyhow::Result<()> {
    let excess_blob_gas = mine_child_block_excess_blob_gas(Some(1)).await?;

    assert_eq!(excess_blob_gas, CHILD_BLOCK_EXCESS_BLOB_GAS);

    Ok(())
}
