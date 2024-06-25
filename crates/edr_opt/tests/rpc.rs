#![cfg(feature = "test-remote")]

use edr_defaults::CACHE_DIR;
use edr_eth::PreEip1898BlockSpec;
use edr_opt::OptimismChainSpec;
use edr_rpc_eth::client::EthRpcClient;
use edr_test_utils::env::get_alchemy_url;

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

    println!("serialized: {block:?}");

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn block_with_deposit_transaction() -> anyhow::Result<()> {
    const BLOCK_NUMBER_WITH_DEPOSIT: u64 = 121_874_088;

    let url = get_alchemy_url().replace("eth-", "opt-");
    let rpc_client = EthRpcClient::<OptimismChainSpec>::new(&url, CACHE_DIR.into(), None)?;

    let block = rpc_client
        .get_block_by_number_with_transaction_data(PreEip1898BlockSpec::Number(
            BLOCK_NUMBER_WITH_DEPOSIT,
        ))
        .await?;

    println!("serialized: {block:?}");

    Ok(())
}
