#![cfg(feature = "test-remote")]

use std::sync::Arc;

use anyhow::anyhow;
use edr_defaults::CACHE_DIR;
use edr_eth::{HashMap, PreEip1898BlockSpec, B256};
use edr_evm::{
    blockchain::ForkedBlockchain, state::IrregularState, Block, RandomHashGenerator, RemoteBlock,
};
use edr_opt::{hardfork, transaction, OptimismChainSpec};
use edr_rpc_eth::client::EthRpcClient;
use edr_test_utils::env::get_alchemy_url;
use revm::{optimism::OptimismSpecId, primitives::b256};
use tokio::runtime;

#[tokio::test(flavor = "multi_thread")]
async fn block_with_transactions() -> anyhow::Result<()> {
    const BLOCK_NUMBER_WITH_TRANSACTIONS: u64 = 117_156_000;

    let url = get_alchemy_url().replace("eth-", "opt-");
    let rpc_client = EthRpcClient::<OptimismChainSpec>::new(&url, CACHE_DIR.into(), None)?;

    let block = rpc_client
        .get_block_by_number_with_transaction_data(PreEip1898BlockSpec::Number(
            BLOCK_NUMBER_WITH_TRANSACTIONS,
        ))
        .await?;

    let block = RemoteBlock::new(block, Arc::new(rpc_client), runtime::Handle::current())?;

    println!("serialized: {block:?}");

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn block_with_deposit_transaction() -> anyhow::Result<()> {
    const BLOCK_NUMBER_WITH_DEPOSIT: u64 = 121_874_088;
    const CHAIN_ID: u64 = 10;
    const SPEC_ID: OptimismSpecId = OptimismSpecId::ECOTONE;

    let runtime = tokio::runtime::Handle::current();

    let url = get_alchemy_url().replace("eth-", "opt-");
    let rpc_client = EthRpcClient::<OptimismChainSpec>::new(&url, CACHE_DIR.into(), None)?;
    let rpc_client = Arc::new(rpc_client);

    let replay_block = {
        let block = rpc_client
            .get_block_by_number_with_transaction_data(PreEip1898BlockSpec::Number(
                BLOCK_NUMBER_WITH_DEPOSIT,
            ))
            .await?;

        RemoteBlock::new(block, rpc_client.clone(), runtime.clone())?
    };

    let mut irregular_state = IrregularState::default();
    let state_root_generator = Arc::new(parking_lot::Mutex::new(RandomHashGenerator::with_seed(
        edr_defaults::STATE_ROOT_HASH_SEED,
    )));
    let hardfork_activation_overrides = HashMap::new();

    let hardfork_activations =
        hardfork::chain_hardfork_activations(CHAIN_ID).ok_or(anyhow!("Unsupported chain id"))?;

    let spec_id = hardfork_activations
        .hardfork_at_block(BLOCK_NUMBER_WITH_DEPOSIT, replay_block.header().timestamp)
        .ok_or(anyhow!("Unsupported block"));

    let blockchain = ForkedBlockchain::new(
        runtime.clone(),
        None,
        SPEC_ID,
        rpc_client.clone(),
        Some(BLOCK_NUMBER_WITH_DEPOSIT - 1),
        &mut irregular_state,
        state_root_generator,
        &hardfork_activation_overrides,
    )
    .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn deposit_transaction() -> anyhow::Result<()> {
    const TRANSACTION_HASH: B256 =
        b256!("cca2f31992022e3a833959c505de021285a7c5339c8d1b8ad75100074e1c6aea");

    let url = get_alchemy_url().replace("eth-", "opt-");
    let rpc_client = EthRpcClient::<OptimismChainSpec>::new(&url, CACHE_DIR.into(), None)?;

    let transaction = rpc_client
        .get_transaction_by_hash(TRANSACTION_HASH)
        .await?
        .expect("Transaction must exist");

    let transaction = transaction::Signed::try_from(transaction)?;
    assert!(matches!(transaction, transaction::Signed::Deposited(_)));

    Ok(())
}
